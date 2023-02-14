use crate::error::Error;

pub fn authenticate(username: &str) -> Result<(), Error> {
    let mut pam = sudo_pam::PamContext::new_cli()
        .target_user(username)
        .service_name("sudo")
        .build()?;

    pam.authenticate(false, true)?;
    pam.validate_account(false, true)?;

    Ok(())
}
