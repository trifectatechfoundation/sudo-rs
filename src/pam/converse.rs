use std::{io, time::Duration};

use crate::cutils::string_from_ptr;

use super::sys::*;

use super::{error::PamResult, rpassword, securemem::PamBuffer, PamError, PamErrorType};

/// Each message in a PAM conversation will have a message style. Each of these
/// styles must be handled separately.
#[derive(Clone, Copy)]
pub enum PamMessageStyle {
    /// Prompt for input using a message. The input should considered secret
    /// and should be hidden from view.
    PromptEchoOff = PAM_PROMPT_ECHO_OFF as isize,
    /// Prompt for input using a message. The input does not have to be
    /// considered a secret and may be displayed to the user.
    PromptEchoOn = PAM_PROMPT_ECHO_ON as isize,
    /// Display an error message. The user should not be prompted for any input.
    ErrorMessage = PAM_ERROR_MSG as isize,
    /// Display some informational text. The user should not be prompted for any
    /// input.
    TextInfo = PAM_TEXT_INFO as isize,
}

impl PamMessageStyle {
    pub fn from_int(val: libc::c_int) -> Option<PamMessageStyle> {
        use PamMessageStyle::*;

        match val as _ {
            PAM_PROMPT_ECHO_OFF => Some(PromptEchoOff),
            PAM_PROMPT_ECHO_ON => Some(PromptEchoOn),
            PAM_ERROR_MSG => Some(ErrorMessage),
            PAM_TEXT_INFO => Some(TextInfo),
            _ => None,
        }
    }
}

pub trait Converser {
    /// Handle a normal prompt, i.e. present some message and ask for a value.
    /// The value is not considered a secret.
    fn handle_normal_prompt(&self, msg: &str) -> PamResult<PamBuffer>;

    /// Handle a hidden prompt, i.e. present some message and ask for a value.
    /// The value is considered secret and should not be visible.
    fn handle_hidden_prompt(&self, msg: &str) -> PamResult<PamBuffer>;

    /// Display an error message to the user, the user does not need to input a
    /// value.
    fn handle_error(&self, msg: &str) -> PamResult<()>;

    /// Display an informational message to the user, the user does not need to
    /// input a value.
    fn handle_info(&self, msg: &str) -> PamResult<()>;
}

/// Handle a single message in a conversation.
fn handle_message<C: Converser>(
    app_data: &ConverserData<C>,
    style: PamMessageStyle,
    msg: &str,
) -> PamResult<Option<PamBuffer>> {
    use PamMessageStyle::*;

    match style {
        PromptEchoOn => {
            if app_data.no_interact {
                return Err(PamError::InteractionRequired);
            }
            app_data.converser.handle_normal_prompt(msg).map(Some)
        }
        PromptEchoOff => {
            if app_data.no_interact {
                return Err(PamError::InteractionRequired);
            }
            let final_prompt = match app_data.auth_prompt.as_deref() {
                None => {
                    // Suppress password prompt entirely when -p '' is passed.
                    String::new()
                }
                Some(prompt) => {
                    format!("[{}: {prompt}] {msg}", app_data.converser_name)
                }
            };
            app_data
                .converser
                .handle_hidden_prompt(&final_prompt)
                .map(Some)
        }
        ErrorMessage => app_data.converser.handle_error(msg).map(|()| None),
        TextInfo => app_data.converser.handle_info(msg).map(|()| None),
    }
}

/// A converser that uses stdin/stdout/stderr to display messages and to request
/// input from the user.
pub struct CLIConverser {
    pub(super) name: String,
    pub(super) use_stdin: bool,
    pub(super) bell: bool,
    pub(super) password_feedback: bool,
    pub(super) password_timeout: Option<Duration>,
}

use rpassword::Terminal;

impl CLIConverser {
    fn open(&self) -> std::io::Result<Terminal<'_>> {
        if self.use_stdin {
            Terminal::open_stdie()
        } else {
            Terminal::open_tty()
        }
    }
}

impl Converser for CLIConverser {
    fn handle_normal_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
        let mut tty = self.open()?;
        tty.prompt(&format!("[{}: input needed] {msg} ", self.name))?;
        Ok(tty.read_cleartext()?)
    }

    fn handle_hidden_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
        let mut tty = self.open()?;
        if self.bell && !self.use_stdin {
            tty.bell()?;
        }
        tty.prompt(msg)?;
        if self.password_feedback {
            tty.read_password_with_feedback(self.password_timeout)
        } else {
            tty.read_password(self.password_timeout)
        }
        .map_err(|err| {
            if let io::ErrorKind::TimedOut = err.kind() {
                PamError::TimedOut
            } else {
                PamError::IoError(err)
            }
        })
    }

    fn handle_error(&self, msg: &str) -> PamResult<()> {
        let mut tty = self.open()?;
        Ok(tty.prompt(&format!("[{} error] {msg}\n", self.name))?)
    }

    fn handle_info(&self, msg: &str) -> PamResult<()> {
        let mut tty = self.open()?;
        Ok(tty.prompt(&format!("[{}] {msg}\n", self.name))?)
    }
}

/// Helper struct that contains the converser as well as panic boolean
pub(super) struct ConverserData<C> {
    pub(super) converser: C,
    pub(super) converser_name: String,
    pub(super) no_interact: bool,
    pub(super) auth_prompt: Option<String>,
    // pam_authenticate does not return error codes returned by the conversation
    // function; these are set by the conversation function instead of returning
    // multiple error codes.
    pub(super) timed_out: bool,
    pub(super) panicked: bool,
}

/// This function implements the conversation function of `pam_conv`.
///
/// This function should always be called with an appdata_ptr that implements
/// the `Converser` trait. It then collects the messages provided into a vector
/// that is passed to the converser. The converser can then respond to those
/// messages and add their replies (where applicable). Finally the replies are
/// converted back to the C interface and returned to PAM. This function tries
/// to catch any unwinding panics and sets state to indicate that a panic
/// occurred.
///
/// # Safety
/// * If called with an appdata_ptr that does not correspond with the Converser
///   this function will exhibit undefined behavior.
/// * The messages from PAM are assumed to be formatted correctly.
pub(super) unsafe extern "C" fn converse<C: Converser>(
    num_msg: libc::c_int,
    msg: *mut *const pam_message,
    response: *mut *mut pam_response,
    appdata_ptr: *mut libc::c_void,
) -> libc::c_int {
    let result = std::panic::catch_unwind(|| {
        let mut resp_bufs = Vec::with_capacity(num_msg as usize);
        for i in 0..num_msg as usize {
            // convert the input messages to Rust types
            // SAFETY: the PAM contract ensures that `num_msg` does not exceed the amount
            // of messages presented to this function in `msg`, and that it is not being
            // written to at the same time as we are reading it. Note that the reference
            // we create does not escape this loopy body.
            let message: &pam_message = unsafe { &**msg.add(i) };

            // SAFETY: PAM ensures that the messages passed are properly null-terminated
            let msg = unsafe { string_from_ptr(message.msg) };
            let style = if let Some(style) = PamMessageStyle::from_int(message.msg_style) {
                style
            } else {
                // early return if there is a failure to convert, pam would have given us nonsense
                return PamErrorType::ConversationError;
            };

            // send the conversation off to the Rust part
            // SAFETY: appdata_ptr contains the `*mut ConverserData` that is untouched by PAM
            let app_data = unsafe { &mut *(appdata_ptr as *mut ConverserData<C>) };
            match handle_message(app_data, style, &msg) {
                Ok(resp_buf) => {
                    resp_bufs.push(resp_buf);
                }
                Err(PamError::TimedOut) => {
                    app_data.timed_out = true;
                    return PamErrorType::ConversationError;
                }
                Err(_) => return PamErrorType::ConversationError,
            }
        }

        // Allocate enough memory for the responses, which are initialized with zero.
        // SAFETY: this will either allocate the required amount of (initialized) bytes,
        // or return a null pointer.
        let temp_resp = unsafe {
            libc::calloc(
                num_msg as libc::size_t,
                std::mem::size_of::<pam_response>() as libc::size_t,
            )
        } as *mut pam_response;
        if temp_resp.is_null() {
            return PamErrorType::BufferError;
        }

        // Store the responses
        for (i, resp_buf) in resp_bufs.into_iter().enumerate() {
            // SAFETY: `i` will not exceed `num_msg` by the way `conversation_messages`
            // is constructed, so `temp_resp` will have allocated-and-initialized data at
            // the required offset that only we have a writable pointer to.
            let response: &mut pam_response = unsafe { &mut *(temp_resp.add(i)) };

            if let Some(secbuf) = resp_buf {
                response.resp = secbuf.leak().as_ptr().cast();
            }
        }

        // Set the responses
        // SAFETY: PAM contract says that we are passed a valid, non-null, writeable pointer here.
        unsafe { *response = temp_resp };

        PamErrorType::Success
    });

    // handle any unwinding panics that occurred here
    let res = match result {
        Ok(r) => r,
        Err(_) => {
            // notify caller that a panic has occurred
            // SAFETY: appdata_ptr contains the `*mut ConverserData` that is untouched by PAM
            let app_data = unsafe { &mut *(appdata_ptr as *mut ConverserData<C>) };
            app_data.panicked = true;

            PamErrorType::ConversationError
        }
    };
    res.as_int()
}

#[allow(clippy::undocumented_unsafe_blocks)]
#[cfg(test)]
mod test {
    use super::*;
    use std::pin::Pin;
    use PamMessageStyle::*;

    struct PamMessage {
        msg: String,
        style: PamMessageStyle,
    }

    impl Converser for String {
        fn handle_normal_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
            Ok(PamBuffer::new(format!("{self} says {msg}").into_bytes()))
        }

        fn handle_hidden_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
            Ok(PamBuffer::new(msg.as_bytes().to_vec()))
        }

        fn handle_error(&self, msg: &str) -> PamResult<()> {
            panic!("{msg}")
        }

        fn handle_info(&self, _msg: &str) -> PamResult<()> {
            Ok(())
        }
    }

    // essentially do the inverse of the "conversation function"
    fn dummy_pam(msgs: &[PamMessage], talkie: &pam_conv) -> Vec<Option<String>> {
        let pam_msgs = msgs
            .iter()
            .map(|PamMessage { msg, style, .. }| pam_message {
                msg: std::ffi::CString::new(&msg[..]).unwrap().into_raw(),
                msg_style: *style as i32,
            })
            .rev()
            .collect::<Vec<pam_message>>();
        let mut ptrs = pam_msgs
            .iter()
            .map(|x| x as *const pam_message)
            .rev()
            .collect::<Vec<*const pam_message>>();

        let mut raw_response = std::ptr::null_mut::<pam_response>();
        let conv_err = unsafe {
            talkie.conv.expect("non-null fn ptr")(
                ptrs.len() as i32,
                ptrs.as_mut_ptr(),
                &mut raw_response,
                talkie.appdata_ptr,
            )
        };

        // deallocate the leaky strings
        for rec in ptrs {
            unsafe {
                drop(std::ffi::CString::from_raw((*rec).msg as *mut _));
            }
        }
        if conv_err != 0 {
            return vec![];
        }

        let result = msgs
            .iter()
            .enumerate()
            .map(|(i, _)| unsafe {
                let ptr = raw_response.add(i);
                if (*ptr).resp.is_null() {
                    None
                } else {
                    // "The resp_retcode member of this struct is unused and should be set to zero."
                    assert_eq!((*ptr).resp_retcode, 0);
                    let response = string_from_ptr((*ptr).resp);
                    libc::free((*ptr).resp as *mut _);
                    Some(response)
                }
            })
            .collect();

        unsafe { libc::free(raw_response as *mut _) };
        result
    }

    fn msg(style: PamMessageStyle, msg: &str) -> PamMessage {
        let msg = msg.to_string();
        PamMessage { style, msg }
    }

    // sanity check on the test cases; lib.rs is expected to manage the lifetime of the pointer
    // inside the pam_conv object explicitly.

    use std::marker::PhantomData;
    struct PamConvBorrow<'a> {
        pam_conv: pam_conv,
        _marker: std::marker::PhantomData<&'a ()>,
    }

    impl<'a> PamConvBorrow<'a> {
        fn new<C: Converser>(data: Pin<&'a mut ConverserData<C>>) -> PamConvBorrow<'a> {
            let appdata_ptr =
                unsafe { data.get_unchecked_mut() as *mut ConverserData<C> as *mut libc::c_void };
            PamConvBorrow {
                pam_conv: pam_conv {
                    conv: Some(converse::<C>),
                    appdata_ptr,
                },
                _marker: PhantomData,
            }
        }

        fn borrow(&self) -> &pam_conv {
            &self.pam_conv
        }
    }

    #[test]
    fn miri_pam_gpt() {
        let mut hello = Box::pin(ConverserData {
            converser: "tux".to_string(),
            converser_name: "tux".to_string(),
            no_interact: false,
            auth_prompt: Some("authenticate".to_owned()),
            timed_out: false,
            panicked: false,
        });
        let cookie = PamConvBorrow::new(hello.as_mut());
        let pam_conv = cookie.borrow();

        assert_eq!(dummy_pam(&[], pam_conv), vec![]);

        assert_eq!(
            dummy_pam(&[msg(PromptEchoOn, "hello")], pam_conv),
            vec![Some("tux says hello".to_string())]
        );

        assert_eq!(
            dummy_pam(&[msg(PromptEchoOff, "fish")], pam_conv),
            vec![Some("[tux: authenticate] fish".to_string())]
        );

        assert_eq!(dummy_pam(&[msg(TextInfo, "mars")], pam_conv), vec![None]);

        assert_eq!(
            dummy_pam(
                &[
                    msg(PromptEchoOff, "banging the rocks together"),
                    msg(TextInfo, ""),
                    msg(PromptEchoOn, ""),
                ],
                pam_conv
            ),
            vec![
                Some("[tux: authenticate] banging the rocks together".to_string()),
                None,
                Some("tux says ".to_string()),
            ]
        );

        //assert!(!hello.panicked); // not allowed by borrow checker
        let real_hello = unsafe { &mut *(pam_conv.appdata_ptr as *mut ConverserData<String>) };
        assert!(!real_hello.panicked);

        assert_eq!(dummy_pam(&[msg(ErrorMessage, "oops")], pam_conv), vec![]);

        assert!(hello.panicked); // allowed now
    }
}
