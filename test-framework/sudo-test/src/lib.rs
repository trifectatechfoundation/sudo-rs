//! sudo-rs test framework

#![deny(missing_docs)]
#![deny(unsafe_code)]

use std::{
    collections::{HashMap, HashSet},
    env,
    path::Path,
    sync::Once,
};

use bimap::BiMap;
use docker::Container;

pub use docker::{Command, Output};

type Error = Box<dyn std::error::Error>;
type Result<T> = core::result::Result<T, Error>;

mod docker;

const BASE_IMAGE: &str = env!("CARGO_CRATE_NAME");

/// are we testing the original sudo?
pub fn is_original_sudo() -> bool {
    matches!(SudoUnderTest::from_env(), Ok(SudoUnderTest::Theirs))
}

/// test environment builder
#[derive(Default)]
pub struct EnvBuilder {
    pam_d_sudo: Option<String>,
    sudoers: String,
    sudoers_chmod: Option<String>,
    sudoers_chown: Option<String>,
    text_files: HashMap<String, TextFile>,
    username_to_groups: HashMap<String, HashSet<String>>,
    username_to_passwords: HashMap<String, String>,
    user_id_to_username: BiMap<u32, String>,
    group_id_to_groupname: BiMap<u32, String>,
}

struct TextFile {
    contents: String,
    chmod: String,
    chown: String,
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

    /// assign a known user ID to a user instead of relying on runtime allocation
    pub fn user_id(&mut self, username: &str, user_id: u32) -> &mut Self {
        // > 1000 because we want to avoid collisions with system user ids
        // 100 should be enough for testing purposes
        assert!(
            (1000..=1100).contains(&user_id),
            "user ID range is limited to 1000-1100"
        );

        assert!(
            !self.user_id_to_username.contains_left(&user_id),
            "user ID {user_id} has already been assigned"
        );

        self.user_id_to_username
            .insert(user_id, username.to_string());

        self
    }

    /// assign a known group ID to a group instead of relying on runtime allocation
    pub fn group_id(&mut self, groupname: &str, group_id: u32) -> &mut Self {
        // > 1000 because we want to avoid collisions with system group ids
        // 100 should be enough for testing purposes
        assert!(
            (1000..=1100).contains(&group_id),
            "group ID range is limited to 1000-1100"
        );

        assert!(
            !self.group_id_to_groupname.contains_left(&group_id),
            "group ID {group_id} has already been assigned"
        );

        self.group_id_to_groupname
            .insert(group_id, groupname.to_string());

        self
    }

    /// assigns the given `password` to the specified user
    ///
    /// NOTE by default users have no password
    pub fn user_password(&mut self, username: &str, password: &str) -> &mut Self {
        self.username_to_passwords
            .insert(username.to_string(), password.to_string());
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

    const DEFAULT_PAM_D_SUDO: &str = r#"#%PAM-1.0

@include common-auth
@include common-account
@include common-session-noninteractive"#;

    /// overwrites the contents of `/etc/pam.d/sudo`
    ///
    /// if this method is not called the contents of `/etc/pam.d/sudo` will match the contents of
    /// the file provided by the `sudo` package
    pub fn pam_d_sudo(&mut self, pam_conf: &str) -> &mut Self {
        let pam_d_sudo = self.pam_d_sudo.get_or_insert_with(String::new);
        pam_d_sudo.push_str(pam_conf);
        pam_d_sudo.push('\n');
        self
    }

    const DEFAULT_SUDOERS_CHOWN: &str = "root:root";

    /// NOTE if unset, defaults to "root:root"
    pub fn sudoers_chown(&mut self, chown: &str) -> &mut Self {
        assert!(self.sudoers_chown.is_none(), "sudoers_chown already set");
        self.sudoers_chown = Some(chown.to_string());
        self
    }

    const DEFAULT_SUDOERS_CHMOD: &str = "440";

    /// NOTE if unset, defaults to "440"
    pub fn sudoers_chmod(&mut self, chmod: &str) -> &mut Self {
        assert!(self.sudoers_chown.is_none(), "sudoers_chmod already set");
        self.sudoers_chmod = Some(chmod.to_string());
        self
    }

    /// Creates a file at `path` with specified `contents` and permissions
    ///
    /// NOTE `path` must be absolute
    pub fn text_file(&mut self, path: &str, chown: &str, chmod: &str, contents: &str) -> &mut Self {
        assert!(Path::new(path).is_absolute(), "path must be absolute");

        assert!(
            !self.text_files.contains_key(path),
            "text file has already been declared"
        );

        self.text_files.insert(
            path.to_string(),
            TextFile {
                contents: contents.to_string(),
                chmod: chmod.to_string(),
                chown: chown.to_string(),
            },
        );

        self
    }

    /// builds the test environment
    pub fn build(&self) -> Result<Env> {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            docker::build_base_image().expect("fatal error: could not build the base Docker image")
        });

        let container = Container::new(BASE_IMAGE)?;

        let mut users = get_users(&container)?;

        for username in &users {
            assert!(
                !self.user_id_to_username.contains_right(username),
                "cannot override the user ID of system user {username}"
            );
        }

        for username in self.user_id_to_username.right_values() {
            assert!(
                self.username_to_groups.contains_key(username),
                "cannot assign user ID to non-existent user {username}"
            );
        }

        let mut groups = get_groups(&container)?;

        for groupname in &groups {
            assert!(
                !self.group_id_to_groupname.contains_right(groupname),
                "cannot override the group ID of system group {groupname}"
            );
        }

        for groupname in self.group_id_to_groupname.right_values() {
            assert!(
                !self.username_to_groups.contains_key(groupname),
                "cannot assign group ID to the implicit user group {groupname}"
            );

            assert!(
                self.username_to_groups
                    .values()
                    .any(|groups| groups.contains(groupname)),
                "cannot assign group ID to non-existent group {groupname}"
            );
        }

        // normally this would be done with `visudo` as that uses a file lock but as it's guaranteed
        // that no user is active in the container at this point doing it like this is fine
        let path = "/etc/sudoers";
        container.cp(path, &self.sudoers)?;

        container
            .exec(
                Command::new("chown")
                    .arg(
                        self.sudoers_chown
                            .as_deref()
                            .unwrap_or(Self::DEFAULT_SUDOERS_CHOWN),
                    )
                    .arg(path),
            )?
            .assert_success()?;

        container
            .exec(
                Command::new("chmod")
                    .arg(
                        self.sudoers_chmod
                            .as_deref()
                            .unwrap_or(Self::DEFAULT_SUDOERS_CHMOD),
                    )
                    .arg(path),
            )?
            .assert_success()?;

        let path = "/etc/pam.d/sudo";
        container.cp(
            path,
            self.pam_d_sudo
                .as_deref()
                .unwrap_or(Self::DEFAULT_PAM_D_SUDO),
        )?;

        container
            .exec(Command::new("chown").args(["root:root", path]))?
            .assert_success()?;
        container
            .exec(Command::new("chmod").args(["644", path]))?
            .assert_success()?;

        // create groups with known IDs first to avoid collisions ..
        for (group_id, groupname) in &self.group_id_to_groupname {
            container
                .exec(
                    Command::new("groupadd")
                        .arg("-g")
                        .arg(group_id.to_string())
                        .arg(groupname),
                )?
                .assert_success()?;

            groups.insert(groupname.to_string());
        }

        // .. with groups that get assigned IDs dynamically
        for user_groups in self.username_to_groups.values() {
            for user_group in user_groups {
                if !groups.contains(user_group) {
                    container
                        .exec(Command::new("groupadd").arg(user_group))?
                        .assert_success()?;

                    groups.insert(user_group.to_string());
                }
            }
        }

        // create users with known IDs first to avoid collisions ..
        for (user_id, username) in &self.user_id_to_username {
            //
            let user = User {
                name: username,
                id: Some(*user_id),
                groups: self
                    .username_to_groups
                    .get(username)
                    .cloned()
                    .unwrap_or_default(),
            };

            user.create(&container)?;

            users.insert(username.to_string());
            groups.insert(username.to_string());
        }

        // .. with users that get assigned IDs dynamically
        for (username, user_groups) in &self.username_to_groups {
            if users.contains(username) {
                continue;
            }

            let user = User {
                name: username,
                id: None,
                groups: user_groups.clone(),
            };

            user.create(&container)?;

            users.insert(username.to_string());
            groups.insert(username.to_string());
        }

        for (username, password) in &self.username_to_passwords {
            assert!(
                users.contains(username),
                "cannot assign password to non-existing user: {username}"
            );

            container
                .exec(Command::new("chpasswd").stdin(format!("{username}:{password}")))?
                .assert_success()?;
        }

        for (path, text_file) in &self.text_files {
            container.cp(path, &text_file.contents)?;

            container
                .exec(Command::new("chown").args([&text_file.chown, path]))?
                .assert_success()?;
            container
                .exec(Command::new("chmod").args([&text_file.chmod, path]))?
                .assert_success()?;
        }

        Ok(Env { container, users })
    }
}

enum SudoUnderTest {
    Ours,
    Theirs,
}

impl SudoUnderTest {
    fn from_env() -> Result<Self> {
        if let Ok(under_test) = env::var("SUDO_UNDER_TEST") {
            if under_test == "ours" {
                Ok(Self::Ours)
            } else if under_test == "theirs" {
                Ok(Self::Theirs)
            } else {
                Err("variable SUDO_UNDER_TEST must be set to one of: ours, theirs".into())
            }
        } else {
            Ok(Self::Theirs)
        }
    }
}

fn get_groups(container: &Container) -> Result<HashSet<String>> {
    let stdout = container
        .exec(Command::new("getent").arg("group"))?
        .stdout()?;
    let mut groups = HashSet::new();
    for line in stdout.lines() {
        if let Some((name, _rest)) = line.split_once(':') {
            groups.insert(name.to_string());
        }
    }

    Ok(groups)
}

fn get_users(container: &Container) -> Result<HashSet<String>> {
    let stdout = container
        .exec(Command::new("getent").arg("passwd"))?
        .stdout()?;
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
    /// creates a new env builder and uses the specified `sudoers` string as the initial contents of
    /// the `/etc/sudoers` file
    #[allow(clippy::new_ret_no_self)]
    pub fn new(sudoers: &str) -> EnvBuilder {
        let mut builder = EnvBuilder::default();
        builder.sudoers(sudoers);
        builder
    }
}

impl Command {
    /// executes the command in the specified test environment
    pub fn exec(&self, env: &Env) -> Result<Output> {
        if let Some(username) = self.get_user() {
            assert!(
                env.users.contains(username),
                "tried to exec as non-existent user: {username}"
            );
        }

        env.container.exec(self)
    }
}

struct User<'a> {
    name: &'a str,
    id: Option<u32>,
    groups: HashSet<String>,
}

impl User<'_> {
    fn create(&self, container: &Container) -> Result<()> {
        let mut useradd = Command::new("useradd");
        useradd.arg("-m");

        if let Some(id) = self.id {
            useradd.arg("-u").arg(id.to_string());
        }

        if !self.groups.is_empty() {
            useradd
                .arg("-G")
                .arg(self.groups.iter().cloned().collect::<Vec<_>>().join(","));
        }

        useradd.arg(self.name);

        container.exec(&useradd)?.assert_success()
    }
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

        Command::new("sh")
            .arg("-c")
            .arg("[ -d /home/ferris ]")
            .exec(&env)?
            .assert_success()
    }

    #[test]
    fn created_user_belongs_to_group_named_after_themselves() -> Result<()> {
        let username = "ferris";
        let env = EnvBuilder::default().user(username, &[]).build()?;

        let stdout = Command::new("groups")
            .as_user(username)
            .exec(&env)?
            .stdout()?;
        let groups = stdout.split(' ').collect::<HashSet<_>>();
        assert!(groups.contains(username));

        Ok(())
    }

    #[test]
    fn creating_user_part_of_existing_group_works() -> Result<()> {
        let user = "ferris";
        let group = "users";
        let env = EnvBuilder::default().user(user, &[group]).build()?;

        let stdout = Command::new("groups").as_user(user).exec(&env)?.stdout()?;
        let user_groups = stdout.split(' ').collect::<HashSet<_>>();
        assert!(user_groups.contains(group));

        Ok(())
    }

    #[test]
    fn sudoers_file_get_created_with_expected_contents() -> Result<()> {
        let expected = "Hello, root!";
        let env = Env::new(expected).build()?;

        let actual = Command::new("cat")
            .arg("/etc/sudoers")
            .exec(&env)?
            .stdout()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn default_pam_d_sudo() -> Result<()> {
        let expected = EnvBuilder::DEFAULT_PAM_D_SUDO;
        let env = EnvBuilder::default().build()?;

        let actual = Command::new("cat")
            .arg("/etc/pam.d/sudo")
            .exec(&env)?
            .stdout()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn overwrite_pam_d_sudo() -> Result<()> {
        let expected = "invalid pam.d file";
        let env = EnvBuilder::default().pam_d_sudo(expected).build()?;

        let actual = Command::new("cat")
            .arg("/etc/pam.d/sudo")
            .exec(&env)?
            .stdout()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn text_file_gets_created_with_right_perms() -> Result<()> {
        let chown = "ferris:ferris";
        let chmod = "600";
        let expected_contents = "hello";
        let path = "/root/file";
        let env = EnvBuilder::default()
            .user("ferris", &[])
            .text_file(path, chown, chmod, expected_contents)
            .build()?;

        let actual_contents = Command::new("cat").arg(path).exec(&env)?.stdout()?;
        assert_eq!(expected_contents, &actual_contents);

        let ls_l = Command::new("ls").args(["-l", path]).exec(&env)?.stdout()?;
        assert!(ls_l.starts_with("-rw-------"));
        assert!(ls_l.contains("ferris ferris"));

        Ok(())
    }

    #[test]
    #[should_panic = "cannot override the user ID of system user root"]
    fn override_root_user_id_fails() {
        EnvBuilder::default().user_id("root", 1000).build().unwrap();
    }

    #[test]
    #[should_panic = "cannot override the group ID of system group root"]
    fn override_root_group_id_fails() {
        EnvBuilder::default()
            .group_id("root", 1000)
            .build()
            .unwrap();
    }

    #[test]
    #[should_panic = "cannot assign user ID to non-existent user ferris"]
    fn set_user_id_of_nonexistent_user_fails() {
        EnvBuilder::default()
            .user_id("ferris", 1000)
            .build()
            .unwrap();
    }

    #[test]
    #[should_panic = "cannot assign group ID to the implicit user group ferris"]
    fn set_group_id_of_implicit_user_group_fails() {
        EnvBuilder::default()
            .user("ferris", &[])
            .group_id("ferris", 1000)
            .build()
            .unwrap();
    }

    #[test]
    #[should_panic = "cannot assign group ID to non-existent group rustaceans"]
    fn set_group_id_of_nonexistent_group_fails() {
        EnvBuilder::default()
            .group_id("rustaceans", 1000)
            .build()
            .unwrap();
    }

    #[test]
    fn user_id_override_works() -> Result<()> {
        let expected = 1023;
        let username = "ferris";
        let env = EnvBuilder::default()
            .user(username, &[])
            .user_id(username, expected)
            .build()?;

        let actual = Command::new("id")
            .args(["-u", username])
            .exec(&env)?
            .stdout()?
            .parse()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn group_id_override_works() -> Result<()> {
        let expected = 1023;
        let username = "ferris";
        let groupname = "rustaceans";
        let env = EnvBuilder::default()
            .user(username, &[groupname])
            .group_id(groupname, expected)
            .build()?;

        let stdout = Command::new("getent")
            .args(["group", groupname])
            .exec(&env)?
            .stdout()?;
        let actual = stdout.split(':').nth(2);
        assert_eq!(Some(expected.to_string().as_str()), actual);

        Ok(())
    }
}
