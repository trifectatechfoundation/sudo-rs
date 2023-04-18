use sudo_pam_sys::*;

use crate::{error::PamResult, rpassword, securemem::PamBuffer, PamErrorType};

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

        match val as libc::c_uint {
            PAM_PROMPT_ECHO_OFF => Some(PromptEchoOff),
            PAM_PROMPT_ECHO_ON => Some(PromptEchoOn),
            PAM_ERROR_MSG => Some(ErrorMessage),
            PAM_TEXT_INFO => Some(TextInfo),
            _ => None,
        }
    }
}

/// A PamMessage contains the data in a single message of a pam conversation
/// and contains the response to that message.
pub struct PamMessage {
    pub msg: String,
    pub style: PamMessageStyle,
    response: Option<PamBuffer>,
}

impl PamMessage {
    /// Set a response value to the message.
    pub fn set_response(&mut self, resp: PamBuffer) {
        self.response = Some(resp);
    }

    /// Clear the response to the message.
    pub fn clear_response(&mut self) {
        self.response = None;
    }
}

/// Contains the conversation messages and allows setting responses to
/// each of these messages.
///
/// Note that generally there will only be one message in each conversation
/// because of historical reasons, and instead multiple conversations will
/// be started for individual messages.
pub struct Conversation {
    messages: Vec<PamMessage>,
}

impl Conversation {
    /// Get an iterator of the messages in this conversation.
    pub fn messages(&self) -> impl Iterator<Item = &PamMessage> {
        self.messages.iter()
    }

    /// Get a mutable iterator of the messages in this conversation.
    ///
    /// This can be used to add the resulting values to the messages.
    pub fn messages_mut(&mut self) -> impl Iterator<Item = &mut PamMessage> {
        self.messages.iter_mut()
    }
}

pub trait Converser {
    /// Handle all the message in the given conversation. They may all be
    /// handled in sequence or at the same time if possible.
    fn handle_conversation(&self, conversation: &mut Conversation) -> PamResult<()>;
}

pub trait SequentialConverser: Converser {
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

impl<T> Converser for T
where
    T: SequentialConverser,
{
    fn handle_conversation(&self, conversation: &mut Conversation) -> PamResult<()> {
        use PamMessageStyle::*;

        for msg in conversation.messages_mut() {
            match msg.style {
                PromptEchoOn => {
                    msg.set_response(self.handle_normal_prompt(&msg.msg)?);
                }
                PromptEchoOff => {
                    msg.set_response(self.handle_hidden_prompt(&msg.msg)?);
                }
                ErrorMessage => {
                    self.handle_error(&msg.msg)?;
                }
                TextInfo => {
                    self.handle_info(&msg.msg)?;
                }
            }
        }

        Ok(())
    }
}

/// A converser that uses stdin/stdout/stderr to display messages and to request
/// input from the user.
pub struct CLIConverser {
    pub(crate) use_stdin: bool,
}

use rpassword::Terminal;

impl CLIConverser {
    fn open(&self) -> std::io::Result<Terminal> {
        if self.use_stdin {
            Terminal::open_stdie()
        } else {
            Terminal::open_tty()
        }
    }
}

impl SequentialConverser for CLIConverser {
    fn handle_normal_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
        let mut tty = self.open()?;
        tty.prompt(&format!("[Sudo: input needed] {msg} "))?;
        Ok(tty.read_cleartext()?)
    }

    fn handle_hidden_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
        let mut tty = self.open()?;
        tty.prompt(&format!("[Sudo: authenticate] {msg}"))?;
        Ok(tty.read_password()?)
    }

    fn handle_error(&self, msg: &str) -> PamResult<()> {
        let mut tty = self.open()?;
        Ok(tty.prompt(&format!("[Sudo error] {msg}\n"))?)
    }

    fn handle_info(&self, msg: &str) -> PamResult<()> {
        let mut tty = self.open()?;
        Ok(tty.prompt(&format!("[Sudo] {msg}\n"))?)
    }
}

/// Helper struct that contains the converser as well as panic boolean
pub(crate) struct ConverserData<C> {
    pub(crate) converser: C,
    pub(crate) panicked: bool,
}

/// This function implements the conversation function of `pam_conv`.
///
/// This function should always be called with an appdata_ptr that implements
/// the `Converser` trait. It then collects the messages provided into a vector
/// that is passed to the converser. The converser can then respond to those
/// messages and add their replies (where applicable). Finally the replies are
/// converted back to the C interface and returned to PAM. This function tries
/// to catch any unwinding panics and sets state to indicate that a panic
/// occured.
///
/// # Safety
/// * If called with an appdata_ptr that does not correspond with the Converser
///   this function will exhibit undefined behavior.
/// * The messages from PAM are assumed to be formatted correctly.
pub(crate) unsafe extern "C" fn converse<C: Converser>(
    num_msg: libc::c_int,
    msg: *mut *const pam_message,
    response: *mut *mut pam_response,
    appdata_ptr: *mut libc::c_void,
) -> libc::c_int {
    let result = std::panic::catch_unwind(|| {
        // convert the input messages to Rust types
        let mut conversation = Conversation {
            messages: Vec::with_capacity(num_msg as usize),
        };
        for i in 0..num_msg as isize {
            let message: &pam_message = unsafe { &**msg.offset(i) };

            let msg = unsafe { sudo_cutils::string_from_ptr(message.msg) };
            let style = if let Some(style) = PamMessageStyle::from_int(message.msg_style) {
                style
            } else {
                // early return if there is a failure to convert, pam would have given us nonsense
                return PamErrorType::ConversationError;
            };

            conversation.messages.push(PamMessage {
                msg,
                style,
                response: None,
            });
        }

        // send the conversation of to the Rust part
        let app_data = unsafe { &mut *(appdata_ptr as *mut ConverserData<C>) };
        if app_data
            .converser
            .handle_conversation(&mut conversation)
            .is_err()
        {
            return PamErrorType::ConversationError;
        }

        // Conversation should now contain response messages
        // allocate enough memory for the responses, set it to zero
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
        for (i, msg) in conversation.messages.into_iter().enumerate() {
            let response: &mut pam_response = unsafe { &mut *(temp_resp.add(i)) };

            if let Some(secbuf) = msg.response {
                response.resp = secbuf.leak() as *mut _;
            }
        }

        // Set the responses
        unsafe { *response = temp_resp };

        PamErrorType::Success
    });

    // handle any unwinding panics that occured here
    let res = match result {
        Ok(r) => r,
        Err(_) => {
            // notify caller that a panic has occured
            let app_data = unsafe { &mut *(appdata_ptr as *mut ConverserData<C>) };
            app_data.panicked = true;

            PamErrorType::ConversationError
        }
    };
    res.as_int()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::pin::Pin;
    use PamMessageStyle::*;

    impl SequentialConverser for String {
        fn handle_normal_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
            Ok(PamBuffer::new(format!("{self} says {msg}").into_bytes()))
        }

        fn handle_hidden_prompt(&self, msg: &str) -> PamResult<PamBuffer> {
            Ok(PamBuffer::new(
                format!("{self}s secret is {msg}").into_bytes(),
            ))
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
                    let response = sudo_cutils::string_from_ptr((*ptr).resp);
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
        PamMessage {
            style,
            msg,
            response: None,
        }
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
            vec![Some("tuxs secret is fish".to_string())]
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
                Some("tuxs secret is banging the rocks together".to_string()),
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
