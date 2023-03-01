use std::{
    collections::{HashMap, HashSet},
    fs,
    process::Command,
    sync::Once,
};

use docker::{As, Container, ExecOutput};
use tempfile::TempDir;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

mod docker;
mod helpers;

const BASE_IMAGE: &str = env!("CARGO_CRATE_NAME");

/// test environment builder
#[derive(Default)]
pub struct EnvBuilder {
    sudoers: String,
    sudoers_chmod: Option<String>,
    sudoers_chown: Option<String>,
    username_to_groups: HashMap<String, HashSet<String>>,
}

impl EnvBuilder {
    /// add an user to the environment
    ///
    /// NOTE users will have a home directory at `/home/$username` and will be part of the group
    /// `$username`
    pub fn user(&mut self, username: &str, groups: &[&str]) -> &mut Self {
        assert!(
            !self.username_to_groups.contains_key(username),
            "user `{username}` declared more than once"
        );

        let mut set = HashSet::new();
        for group in groups {
            assert!(
                !set.contains(*group),
                "group `{group}` declared more than once"
            );

            set.insert(group.to_string());
        }

        assert!(!set.contains(username), "do not list $username in $groups");

        self.username_to_groups.insert(username.to_string(), set);

        self
    }

    /// appends content to the `/etc/sudoers` file
    ///
    /// NOTE that if this method is not called `/etc/sudoers` will be empty
    pub fn sudoers(&mut self, sudoers: &str) -> &mut Self {
        self.sudoers.push_str(sudoers);
        self.sudoers.push('\n');
        self
    }

    const DEFAULT_SUDOERS_CHOWN: &str = "root:root";

    /// NOTE defaults to "root:root"
    pub fn sudoers_chown(&mut self, chown: &str) -> &mut Self {
        assert!(self.sudoers_chown.is_none(), "sudoers_chown already set");
        self.sudoers_chown = Some(chown.to_string());
        self
    }

    const DEFAULT_SUDOERS_CHMOD: &str = "440";

    /// NOTE defaults to "440"
    pub fn sudoers_chmod(&mut self, chmod: &str) -> &mut Self {
        assert!(self.sudoers_chown.is_none(), "sudoers_chmod already set");
        self.sudoers_chmod = Some(chmod.to_string());
        self
    }

    pub fn build(&self) -> Result<Env> {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            build_base_image().expect("fatal error: could not build the base Docker image")
        });

        let container = Container::new(BASE_IMAGE)?;

        let mut groups = get_groups(&container)?;
        let mut users = get_users(&container)?;

        // normally this would be done with `visudo` as that uses a file lock but as it's guaranteed
        // that no user is active in the container at this point doing it like this is fine
        let path = "/etc/sudoers";
        container.cp(path, &self.sudoers)?;

        container.stdout(
            &[
                "chown",
                self.sudoers_chown
                    .as_deref()
                    .unwrap_or(Self::DEFAULT_SUDOERS_CHOWN),
                path,
            ],
            As::Root,
        )?;

        container.stdout(
            &[
                "chmod",
                self.sudoers_chmod
                    .as_deref()
                    .unwrap_or(Self::DEFAULT_SUDOERS_CHMOD),
                path,
            ],
            As::Root,
        )?;

        for user_groups in self.username_to_groups.values() {
            for user_group in user_groups {
                if !groups.contains(user_group) {
                    container.stdout(&["groupadd", user_group], As::Root)?;

                    groups.insert(user_group.to_string());
                }
            }
        }

        for (username, user_groups) in &self.username_to_groups {
            let mut cmd = vec!["useradd", "-m", username];
            let group_list;
            if !user_groups.is_empty() {
                group_list = user_groups.iter().cloned().collect::<Vec<_>>().join(",");
                cmd.extend_from_slice(&["-G", &group_list]);
            }
            container.stdout(&cmd, As::Root)?;

            users.insert(username.to_string());
            groups.insert(username.to_string());
        }

        Ok(Env { container, users })
    }
}

fn build_base_image() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_dir = temp_dir.path();

    fs::write(temp_dir.join("Dockerfile"), include_str!("Dockerfile"))?;

    helpers::stdout(
        Command::new("docker")
            .args(["build", "-t", BASE_IMAGE, "."])
            .current_dir(temp_dir),
    )?;

    Ok(())
}

fn get_groups(container: &Container) -> Result<HashSet<String>> {
    let stdout = container.stdout(&["getent", "group"], As::Root)?;
    let mut groups = HashSet::new();
    for line in stdout.lines() {
        if let Some((name, _rest)) = line.split_once(':') {
            groups.insert(name.to_string());
        }
    }

    Ok(groups)
}

fn get_users(container: &Container) -> Result<HashSet<String>> {
    let stdout = container.stdout(&["getent", "passwd"], As::Root)?;
    let mut users = HashSet::new();
    for line in stdout.lines() {
        if let Some((name, _rest)) = line.split_once(':') {
            users.insert(name.to_string());
        }
    }

    Ok(users)
}

/// test environment
pub struct Env {
    container: Container,
    users: HashSet<String>,
}

impl Env {
    pub fn exec(&self, cmd: &[&str], user: As) -> Result<ExecOutput> {
        if let As::User { name } = user {
            assert!(
                self.users.contains(name),
                "tried to exec as non-existing user"
            );
        }

        self.container.exec(cmd, user)
    }
}

macro_rules! assert_contains {
    ($haystack:expr, $needle:expr) => {
        let haystack = &$haystack;
        let needle = &$needle;

        assert!(
            haystack.contains(needle),
            "{haystack:?} did not contain {needle:?}"
        )
    };
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn group_creation_works() -> Result<()> {
        let username = "ferris";
        let groupname = "rustaceans";
        let env = EnvBuilder::default().user(username, &[groupname]).build()?;

        let groups = get_groups(&env.container)?;
        assert!(groups.contains(groupname));

        Ok(())
    }

    #[test]
    fn user_creation_works() -> Result<()> {
        let new_user = "ferris";
        let env = EnvBuilder::default().user(new_user, &[]).build()?;

        let users = get_users(&env.container)?;
        assert!(users.contains(new_user));

        Ok(())
    }

    #[test]
    fn created_user_has_a_home() -> Result<()> {
        let new_user = "ferris";
        let env = EnvBuilder::default().user(new_user, &[]).build()?;

        let output = env.exec(&["sh", "-c", "[ -d /home/ferris ]"], As::Root)?;
        assert!(output.status.success());

        Ok(())
    }

    #[test]
    fn created_user_belongs_to_group_named_after_themselves() -> Result<()> {
        let new_user = "ferris";
        let env = EnvBuilder::default().user(new_user, &[]).build()?;

        let output = env.exec(&["groups"], As::User { name: new_user })?;
        assert!(output.status.success());

        let groups = output.stdout.split(' ').collect::<HashSet<_>>();
        assert!(groups.contains(new_user));

        Ok(())
    }

    #[test]
    fn creating_user_part_of_existing_group_works() -> Result<()> {
        let user = "ferris";
        let group = "users";
        let env = EnvBuilder::default().user(user, &[group]).build()?;

        let output = env.exec(&["groups"], As::User { name: user })?;
        assert!(output.status.success());

        let user_groups = output.stdout.split(' ').collect::<HashSet<_>>();
        dbg!(&user_groups);
        assert!(user_groups.contains(group));

        Ok(())
    }

    #[test]
    fn sudoers_file_get_created_with_expected_contents() -> Result<()> {
        let expected = "Hello, root!";
        let env = EnvBuilder::default().sudoers(expected).build()?;

        let output = env.exec(&["cat", "/etc/sudoers"], As::Root)?;
        assert!(output.status.success());

        let actual = output.stdout;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn cannot_sudo_with_empty_sudoers_file() -> Result<()> {
        let env = EnvBuilder::default().build()?;

        let output = env.exec(&["sudo", "true"], As::Root)?;
        assert_eq!(Some(1), output.status.code());
        assert_contains!(output.stderr, "root is not in the sudoers file");

        Ok(())
    }

    #[test]
    fn cannot_sudo_if_sudoers_file_is_world_writable() -> Result<()> {
        let env = EnvBuilder::default().sudoers_chmod("446").build()?;

        let output = env.exec(&["sudo", "true"], As::Root)?;
        assert_eq!(Some(1), output.status.code());
        assert_contains!(output.stderr, "/etc/sudoers is world writable");

        Ok(())
    }

    #[test]
    fn can_sudo_if_user_is_in_sudoers_file() -> Result<()> {
        let env = EnvBuilder::default()
            .sudoers("root    ALL=(ALL:ALL) ALL")
            .build()?;

        let output = env.exec(&["sudo", "true"], As::Root)?;
        assert!(output.status.success(), "{}", output.stderr);

        Ok(())
    }
}
