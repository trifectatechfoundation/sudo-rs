use sudo_common::error::Error;

pub fn authenticate(username: &str) -> Result<(), Error> {
    let mut pam = sudo_pam::PamContext::builder_cli()
        .target_user(username)
        .service_name("sudo")
        .build()?;

    pam.mark_silent(true);
    pam.mark_allow_null_auth_token(false);

    pam.authenticate()?;
    pam.validate_account()?;

    Ok(())
}
