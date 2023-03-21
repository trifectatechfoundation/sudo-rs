use std::io::{BufRead, Write};

use sudo_pam_sys::*;

use crate::{error::PamResult, PamErrorType};

/// Each message in a PAM conversation will have a message style. Each of these
/// styles must be handled separately.
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
    response: Option<String>,
}

impl PamMessage {
    /// Set a response value to the message.
    pub fn set_response(&mut self, resp: String) {
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
    fn handle_normal_prompt(&self, msg: &str) -> PamResult<String>;

    /// Handle a hidden prompt, i.e. present some message and ask for a value.
    /// The value is considered secret and should not be visible.
    fn handle_hidden_prompt(&self, msg: &str) -> PamResult<String>;

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
pub struct CLIConverser;

impl SequentialConverser for CLIConverser {
    fn handle_normal_prompt(&self, msg: &str) -> PamResult<String> {
        print!("{msg}");
        std::io::stdout().flush().unwrap();

        let mut s = String::new();
        std::io::stdin().lock().read_line(&mut s).unwrap();

        Ok(s)
    }

    fn handle_hidden_prompt(&self, msg: &str) -> PamResult<String> {
        Ok(rpassword::prompt_password(msg)?)
    }

    fn handle_error(&self, msg: &str) -> PamResult<()> {
        eprintln!("{msg}");
        Ok(())
    }

    fn handle_info(&self, msg: &str) -> PamResult<()> {
        println!("{msg}");
        Ok(())
    }
}

/// Helper struct that contains the converser as well as panic boolean
pub(crate) struct ConverserData<C> {
    pub(crate) converser: C,
    pub(crate) panicked: bool,
}

impl<C: Converser> ConverserData<C> {
    /// This function creates a pam_conv struct with the converse function for
    /// the specific converser.
    pub(crate) unsafe fn create_pam_conv(self: std::pin::Pin<&mut Self>) -> pam_conv {
        pam_conv {
            conv: Some(converse::<C>),
            appdata_ptr: self.get_unchecked_mut() as *mut ConverserData<C> as *mut libc::c_void,
        }
    }
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
pub(crate) extern "C" fn converse<C: Converser>(
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
            let message: &pam_message = unsafe { &*((*msg).offset(i)) };

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
        for i in 0..num_msg as isize {
            let response: &mut pam_response = unsafe { &mut *(temp_resp.offset(i)) };

            // Unwrap here should be ok because we previously allocated an array of the same size
            let our_resp = &conversation.messages.get(i as usize).unwrap().response;
            if let Some(r) = our_resp {
                let cstr = unsafe { sudo_cutils::into_leaky_cstring(r) };
                response.resp = cstr as *mut _;
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
