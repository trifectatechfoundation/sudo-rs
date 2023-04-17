use sudo_common::{error::Error, Context};

pub fn authenticate(ctx: &Context) -> Result<(), Error> {
    let mut pam = sudo_pam::PamContext::builder_cli(ctx.stdin)
        .target_user(&ctx.current_user.name)
        .service_name("sudo")
        .build()?;

    pam.mark_silent(true);
    pam.mark_allow_null_auth_token(false);

    pam.authenticate()?;
    pam.validate_account()?;

    Ok(())
}
