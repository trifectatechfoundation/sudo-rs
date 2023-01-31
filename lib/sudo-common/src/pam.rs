use crate::error::Error;

pub fn authenticate(username: &str) -> Result<(), Error> {
    let mut conversation = pam_client::conv_cli::Conversation::new();
    conversation.set_info_prefix("");

    let mut context = pam_client::Context::new("sukkelsudo", Some(username), conversation)
        .map_err(|_| Error::auth("failed to initialize PAM context"))?;

    context
        .authenticate(pam_client::Flag::NONE)
        .map_err(|_| Error::auth("could not authenticate"))?;

    context
        .acct_mgmt(pam_client::Flag::NONE)
        .map_err(|_| Error::auth("account validation failed"))?;

    Ok(())
}
