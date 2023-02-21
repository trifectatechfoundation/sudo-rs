use std::io::{BufRead, Write};

use sudo_pam_sys::*;

use crate::{PamError, PamErrorType};

pub enum PamMessageStyle {
    PromptEchoOff = PAM_PROMPT_ECHO_OFF as isize,
    PromptEchoOn = PAM_PROMPT_ECHO_ON as isize,
    ErrorMessage = PAM_ERROR_MSG as isize,
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

pub struct PamMessage {
    pub msg: String,
    pub style: PamMessageStyle,
    response: Option<String>,
}

impl PamMessage {
    pub fn set_response(&mut self, resp: String) {
        self.response = Some(resp);
    }

    pub fn clear_response(&mut self) {
        self.response = None;
    }
}

pub struct Conversation {
    messages: Vec<PamMessage>,
}

impl Conversation {
    pub fn messages(&self) -> impl Iterator<Item = &PamMessage> {
        self.messages.iter()
    }

    pub fn messages_mut(&mut self) -> impl Iterator<Item = &mut PamMessage> {
        self.messages.iter_mut()
    }
}

pub trait Converser {
    fn handle_conversation(&self, conversation: &mut Conversation) -> Result<(), PamError>;
}

pub trait SequentialConverser: Converser {
    fn handle_normal_prompt(&self, msg: &str) -> Result<String, PamError>;
    fn handle_hidden_prompt(&self, msg: &str) -> Result<String, PamError>;
    fn handle_error(&self, msg: &str) -> Result<(), PamError>;
    fn handle_info(&self, msg: &str) -> Result<(), PamError>;
}

impl<T> Converser for T
where
    T: SequentialConverser,
{
    fn handle_conversation(&self, conversation: &mut Conversation) -> Result<(), PamError> {
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

pub struct CLIConverser;

impl SequentialConverser for CLIConverser {
    fn handle_normal_prompt(&self, msg: &str) -> Result<String, PamError> {
        print!("{msg}");
        std::io::stdout().flush().unwrap();

        let mut s = String::new();
        std::io::stdin().lock().read_line(&mut s).unwrap();

        Ok(s)
    }

    fn handle_hidden_prompt(&self, msg: &str) -> Result<String, PamError> {
        Ok(rpassword::prompt_password(msg)?)
    }

    fn handle_error(&self, msg: &str) -> Result<(), PamError> {
        eprintln!("{msg}");
        Ok(())
    }

    fn handle_info(&self, msg: &str) -> Result<(), PamError> {
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
    pub(crate) unsafe fn create_pam_conv(&mut self) -> pam_conv {
        pam_conv {
            conv: Some(converse::<C>),
            appdata_ptr: self as *mut ConverserData<C> as *mut libc::c_void,
        }
    }
}

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
                let cstr = sudo_cutils::into_leaky_cstring(r);
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
