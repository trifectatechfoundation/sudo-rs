use std::process::exit;

use sudo_cli::SudoOptions;
use sudo_common::{Context, Error};
use sudo_env::environment;
use sudo_exec::ExitReason;
use sudoers::{Authorization, DirChange, Policy, PreJudgementPolicy};

pub trait PolicyPlugin {
    type PreJudgementPolicy: PreJudgementPolicy;
    type Policy: Policy;

    fn init(&mut self) -> Result<Self::PreJudgementPolicy, Error>;
    fn judge(
        &mut self,
        pre: Self::PreJudgementPolicy,
        context: &Context,
    ) -> Result<Self::Policy, Error>;
}

pub trait AuthPlugin {
    fn init(&mut self, context: &Context) -> Result<(), Error>;
    fn authenticate(&mut self, context: &Context) -> Result<(), Error>;
    fn pre_exec(&mut self, context: &Context) -> Result<(), Error>;
    fn cleanup(&mut self);
}

pub struct Pipeline<Policy: PolicyPlugin, Auth: AuthPlugin> {
    pub policy: Policy,
    pub authenticator: Auth,
}

impl<Policy: PolicyPlugin, Auth: AuthPlugin> Pipeline<Policy, Auth> {
    pub fn run(&mut self, sudo_options: SudoOptions) -> Result<(), Error> {
        let pre = self.policy.init()?;
        let secure_path: String = pre
            .secure_path()
            .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default());
        let mut context = Context::build_from_options(sudo_options, secure_path)?;

        let policy = self.policy.judge(pre, &context)?;
        let authorization = policy.authorization();

        match authorization {
            Authorization::Forbidden => {
                return Err(Error::auth(&format!(
                    "I'm sorry {}. I'm afraid I can't do that",
                    context.current_user.name
                )));
            }
            Authorization::Allowed { must_authenticate } => {
                self.apply_policy_to_context(&mut context, &policy)?;
                self.authenticator.init(&context)?;
                if must_authenticate {
                    self.authenticator.authenticate(&context)?;
                }
            }
        }

        self.authenticator.pre_exec(&context)?;

        // build environment
        let current_env = std::env::vars_os().collect();
        let target_env = environment::get_target_environment(current_env, &context, &policy);

        let pid = context.process.pid;

        // run command and return corresponding exit code
        let (reason, emulate_default_handler) = sudo_exec::run_command(context, target_env)?;

        self.authenticator.cleanup();

        // Run any clean-up code before this line.
        emulate_default_handler();

        match reason {
            ExitReason::Code(code) => exit(code),
            ExitReason::Signal(signal) => {
                sudo_system::kill(pid, signal)?;
            }
        }

        Ok(())
    }

    fn apply_policy_to_context(
        &mut self,
        context: &mut Context,
        policy: &<Policy as PolicyPlugin>::Policy,
    ) -> Result<(), sudo_common::Error> {
        // see if the chdir flag is permitted
        match policy.chdir() {
            DirChange::Any => {}
            DirChange::Strict(optdir) => {
                if context.chdir.is_some() {
                    return Err(Error::auth("no permission")); // TODO better user error messages
                } else {
                    context.chdir = optdir.map(std::path::PathBuf::from)
                }
            }
        }

        Ok(())
    }
}
