//! sudo-rs test framework

#![deny(missing_docs)]
#![deny(unsafe_code)]

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    path::Path,
    sync::Once,
};

use docker::{As, Container};

pub use docker::{Child, Command, Output};

mod docker;
pub mod helpers;

type Error = Box<dyn std::error::Error>;
type Result<T> = core::result::Result<T, Error>;

fn base_image() -> &'static str {
    if is_original_sudo() {
        "sudo-test-og"
    } else {
        "sudo-test-rs"
    }
}

/// are we testing the original sudo?
pub fn is_original_sudo() -> bool {
    matches!(SudoUnderTest::from_env(), Ok(SudoUnderTest::Theirs))
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

type AbsolutePath = String;
type Groupname = String;
type Username = String;

/// test environment        
pub struct Env {
    container: Container,
    users: HashSet<Username>,
}

/// creates a new test environment builder that contains the specified `/etc/sudoers` file
#[allow(non_snake_case)]
pub fn Env(sudoers: impl Into<TextFile>) -> EnvBuilder {
    let mut builder = EnvBuilder::default();
    builder.file("/etc/sudoers", sudoers);
    builder
}

impl Command {
    /// executes the command in the specified test environment
    ///
    /// NOTE that the trailing newline from `stdout` and `stderr` will be removed
    ///
    /// # Panics
    ///
    /// this method panics if the requested `as_user` does not exist in the test environment. to
    /// execute a command as a non-existent user use `Command::as_user_id`
    pub fn output(&self, env: &Env) -> Result<Output> {
        if let Some(As::User(username)) = self.get_as() {
            assert!(
                env.users.contains(username),
                "tried to exec as non-existent user: {username}"
            );
        }

        env.container.output(self)
    }

    /// spawns the command in the specified test environment
    pub fn spawn(&self, env: &Env) -> Result<Child> {
        if let Some(As::User(username)) = self.get_as() {
            assert!(
                env.users.contains(username),
                "tried to exec as non-existent user: {username}"
            );
        }

        env.container.spawn(self)
    }
}

/// test environment builder
#[derive(Default)]
pub struct EnvBuilder {
    directories: BTreeMap<AbsolutePath, Directory>,
    files: HashMap<AbsolutePath, TextFile>,
    groups: HashMap<Groupname, Group>,
    hostname: Option<String>,
    users: HashMap<Username, User>,
}

impl EnvBuilder {
    /// adds a `file` to the test environment at the specified `path`
    ///
    /// # Panics
    ///
    /// - if `path` is not an absolute path
    /// - if `path` has previously been declared
    pub fn file(&mut self, path: impl AsRef<str>, file: impl Into<TextFile>) -> &mut Self {
        let path = path.as_ref();
        assert!(Path::new(path).is_absolute(), "path must be absolute");
        assert!(
            !self.files.contains_key(path),
            "file at {path} has already been declared"
        );

        self.files.insert(path.to_string(), file.into());

        self
    }

    /// adds a `directory` to the test environment
    ///
    /// # Panics
    ///
    /// - if `path` is not an absolute path
    /// - if `path` has previously been declared
    pub fn directory(&mut self, directory: impl Into<Directory>) -> &mut Self {
        let directory = directory.into();
        let path = directory.get_path();
        assert!(
            !self.directories.contains_key(path),
            "directory at {path} has already been declared"
        );
        self.directories.insert(path.to_string(), directory);
        self
    }

    /// adds the specified `group` to the test environment
    ///
    /// # Panics
    ///
    /// - if the `group` has previously been declared
    pub fn group(&mut self, group: impl Into<Group>) -> &mut Self {
        let group = group.into();
        let groupname = &group.name;
        assert!(
            !self.groups.contains_key(groupname),
            "group {} has already been declared",
            groupname
        );
        self.groups.insert(groupname.to_string(), group);

        self
    }

    /// adds the specified `user` to the test environment
    ///
    /// # Panics
    ///
    /// - if the `user` has previously been declared
    pub fn user(&mut self, user: impl Into<User>) -> &mut Self {
        let user = user.into();
        let username = &user.name;
        assert!(
            !self.users.contains_key(username),
            "user {} has already been declared",
            username
        );
        self.users.insert(username.to_string(), user);

        self
    }

    /// Sets the hostname of the container to the specified string
    pub fn hostname(&mut self, hostname: impl AsRef<str>) -> &mut Self {
        self.hostname = Some(hostname.as_ref().to_string());
        self
    }

    /// builds the test environment
    ///
    /// # Panics
    ///
    /// - if any specified `user` already exists in the base image
    /// - if any specified `group` already exists in the base image
    /// - if any specified `user` tries to use a user ID that already exists in the base image
    /// - if any specified `group` tries to use a group ID that already exists in the base image
    pub fn build(&self) -> Result<Env> {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            docker::build_base_image().expect("fatal error: could not build the base Docker image")
        });

        let container = Container::new_with_hostname(base_image(), self.hostname.as_deref())?;

        let (mut usernames, user_ids) = getent_passwd(&container)?;

        for new_user in self.users.values() {
            assert!(
                !usernames.contains(&new_user.name),
                "user {} already exists in base image",
                new_user.name
            );

            if let Some(user_id) = new_user.id {
                assert!(
                    !user_ids.contains(&user_id),
                    "user ID {user_id} already exists in base image"
                );
            }
        }

        let (groupnames, group_ids) = getent_group(&container)?;

        for new_group in self.groups.values() {
            assert!(
                !groupnames.contains(&new_group.name),
                "group {} already exists in base image",
                new_group.name
            );

            if let Some(group_id) = new_group.id {
                assert!(
                    !group_ids.contains(&group_id),
                    "group ID {group_id} already exists in base image"
                );
            }
        }

        // create groups with known IDs first to avoid collisions ..
        for group in self.groups.values().filter(|group| group.id.is_some()) {
            group.create(&container)?;
        }

        // .. with groups that get assigned IDs dynamically
        for group in self.groups.values().filter(|group| group.id.is_none()) {
            group.create(&container)?;
        }

        // create users with known IDs first to avoid collisions ..
        for user in self.users.values().filter(|user| user.id.is_some()) {
            user.create(&container)?;
            usernames.insert(user.name.to_string());
        }

        // .. with users that get assigned IDs dynamically
        for user in self.users.values().filter(|user| user.id.is_none()) {
            user.create(&container)?;
            usernames.insert(user.name.to_string());
        }

        for directory in self.directories.values() {
            directory.create(&container)?;
        }

        for (path, file) in &self.files {
            file.create(path, &container)?;
        }

        Ok(Env {
            container,
            users: usernames,
        })
    }
}

/// a user
pub struct User {
    name: Username,

    create_home_directory: bool,
    groups: HashSet<Groupname>,
    id: Option<u32>,
    password: Option<String>,
    shell: Option<String>,
}

/// creates a new user with the specified `name` and the following defaults:
///
/// - on Debian containers, primary group = `users` (GID=100)
/// - automatically assigned user ID
/// - no assigned secondary groups
/// - no assigned password
/// - home directory set to `/home/<name>` but not automatically created
#[allow(non_snake_case)]
pub fn User(name: impl AsRef<str>) -> User {
    name.as_ref().into()
}

impl User {
    /// assigns this user to the specified *secondary* `group`
    ///
    /// NOTE on Debian containers, all new users will be assigned to the `users` primary group (GID=100)
    pub fn secondary_group(mut self, group: impl AsRef<str>) -> Self {
        let groupname = group.as_ref();
        assert!(
            !self.groups.contains(groupname),
            "user {} has already been assigned to {groupname}",
            self.name
        );

        self.groups.insert(groupname.to_string());

        self
    }

    /// assigns this user to all the specified *secondary* `groups`
    ///
    /// NOTE on Debian containers, all new users will be assigned to the `users` primary group (GID=100)
    pub fn secondary_groups(mut self, groups: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        for group in groups {
            self = self.secondary_group(group);
        }
        self
    }

    /// assigns the specified user `id` to this user
    ///
    /// if not specified, the user will get an automatically allocated ID
    pub fn id(mut self, id: u32) -> Self {
        self.id = Some(id);
        self
    }

    /// assigns the specified `password` to this user
    ///
    /// if not specified, the user will have no password
    pub fn password(mut self, password: impl AsRef<str>) -> Self {
        self.password = Some(password.as_ref().to_string());
        self
    }

    /// creates a home directory for the user at `/home/<username>`
    ///
    /// by default, the directory is not created
    pub fn create_home_directory(mut self) -> Self {
        self.create_home_directory = true;
        self
    }

    /// sets the user's shell to the one at the specified `path`
    pub fn shell(mut self, path: impl AsRef<str>) -> Self {
        self.shell = Some(path.as_ref().to_string());
        self
    }

    fn create(&self, container: &Container) -> Result<()> {
        let mut useradd = Command::new("useradd");
        useradd.arg("--no-user-group");
        if self.create_home_directory {
            useradd.arg("--create-home");
        }
        if let Some(path) = &self.shell {
            useradd.arg("--shell").arg(path);
        }
        if let Some(id) = self.id {
            useradd.arg("--uid").arg(id.to_string());
        }
        if !self.groups.is_empty() {
            let group_list = self.groups.iter().cloned().collect::<Vec<_>>().join(",");
            useradd.arg("--groups").arg(group_list);
        }
        useradd.arg(&self.name);
        container.output(&useradd)?.assert_success()?;

        if let Some(password) = &self.password {
            container
                .output(Command::new("chpasswd").stdin(format!("{}:{password}", self.name)))?
                .assert_success()?;
        }

        Ok(())
    }
}

impl From<String> for User {
    fn from(name: String) -> Self {
        assert!(!name.is_empty(), "user name cannot be an empty string");

        Self {
            create_home_directory: false,
            groups: HashSet::new(),
            id: None,
            name,
            password: None,
            shell: None,
        }
    }
}

impl From<&'_ str> for User {
    fn from(name: &'_ str) -> Self {
        name.to_string().into()
    }
}

/// a group
pub struct Group {
    name: Groupname,

    id: Option<u32>,
}

/// creates a group with the specified `name`
#[allow(non_snake_case)]
pub fn Group(name: impl AsRef<str>) -> Group {
    name.as_ref().into()
}

impl Group {
    /// assigns the specified group `id` to this group
    ///
    /// if not specified, the group will get an automatically allocated ID
    pub fn id(mut self, id: u32) -> Self {
        self.id = Some(id);
        self
    }

    fn create(&self, container: &Container) -> Result<()> {
        let mut groupadd = Command::new("groupadd");
        if let Some(id) = self.id {
            groupadd.arg("--gid");
            groupadd.arg(id.to_string());
        }
        groupadd.arg(&self.name);
        container.output(&groupadd)?.assert_success()
    }
}

impl From<String> for Group {
    fn from(name: String) -> Self {
        assert!(!name.is_empty(), "group name cannot be an empty string");

        Self { name, id: None }
    }
}

impl From<&'_ str> for Group {
    fn from(name: &'_ str) -> Self {
        name.to_string().into()
    }
}

/// a text file
pub struct TextFile {
    contents: String,
    trailing_newline: bool,

    chmod: String,
    chown: String,
}

/// creates a text file with the specified `contents`
///
/// NOTE by default, a trailing newline will be appended to the contents if it doesn't contain one.
/// to omit the trailing newline use the `TextFile::no_trailing_newline` method
#[allow(non_snake_case)]
pub fn TextFile(contents: impl AsRef<str>) -> TextFile {
    contents.as_ref().into()
}

impl TextFile {
    const DEFAULT_CHMOD: &str = "000";
    const DEFAULT_CHOWN: &str = "root:root";

    /// chmod string to apply to the file
    ///
    /// if not specified, the default is "000"
    pub fn chmod(mut self, chmod: impl AsRef<str>) -> Self {
        self.chmod = chmod.as_ref().to_string();
        self
    }

    /// chown string to apply to the file
    ///
    /// if not specified, the default is "root:root"
    pub fn chown(mut self, chown: impl AsRef<str>) -> Self {
        self.chown = chown.as_ref().to_string();
        self
    }

    /// strips newlines from the end of the file
    pub fn no_trailing_newline(mut self) -> Self {
        self.trailing_newline = false;
        self
    }

    fn create(&self, path: &str, container: &Container) -> Result<()> {
        let mut contents = self.contents.clone();

        if self.trailing_newline {
            if !contents.ends_with('\n') {
                contents.push('\n');
            }
        } else if contents.ends_with('\n') {
            contents.pop();
        }

        container.cp(path, &contents)?;

        container
            .output(Command::new("chown").args([&self.chown, path]))?
            .assert_success()?;
        container
            .output(Command::new("chmod").args([&self.chmod, path]))?
            .assert_success()
    }
}

impl From<String> for TextFile {
    fn from(contents: String) -> Self {
        Self {
            contents,
            chmod: Self::DEFAULT_CHMOD.to_string(),
            chown: Self::DEFAULT_CHOWN.to_string(),
            trailing_newline: true,
        }
    }
}

impl From<&'_ str> for TextFile {
    fn from(contents: &'_ str) -> Self {
        contents.to_string().into()
    }
}

impl<S: AsRef<str>, const N: usize> From<[S; N]> for TextFile {
    fn from(contents: [S; N]) -> Self {
        let mut buf = String::new();
        for s in contents {
            buf += s.as_ref();
            buf += "\n";
        }

        buf.into()
    }
}

/// creates a directory at the specified `path`
#[allow(non_snake_case)]
pub fn Directory(path: impl AsRef<str>) -> Directory {
    Directory::from(path.as_ref())
}

/// a directory
pub struct Directory {
    path: String,
    chmod: String,
    chown: String,
}

impl Directory {
    const DEFAULT_CHMOD: &str = "100";
    const DEFAULT_CHOWN: &str = "root:root";

    /// chmod string to apply to the file
    ///
    /// if not specified, the default is "000"
    pub fn chmod(mut self, chmod: impl AsRef<str>) -> Self {
        self.chmod = chmod.as_ref().to_string();
        self
    }

    /// chown string to apply to the file
    ///
    /// if not specified, the default is "root:root"
    pub fn chown(mut self, chown: impl AsRef<str>) -> Self {
        self.chown = chown.as_ref().to_string();
        self
    }

    fn get_path(&self) -> &str {
        &self.path
    }

    fn create(&self, container: &Container) -> Result<()> {
        let path = &self.path;
        container
            .output(Command::new("mkdir").args([path]))?
            .assert_success()?;
        container
            .output(Command::new("chown").args([&self.chown, path]))?
            .assert_success()?;
        container
            .output(Command::new("chmod").args([&self.chmod, path]))?
            .assert_success()
    }
}

impl From<String> for Directory {
    fn from(path: String) -> Self {
        Self {
            path,
            chmod: Self::DEFAULT_CHMOD.to_string(),
            chown: Self::DEFAULT_CHOWN.to_string(),
        }
    }
}

impl From<&'_ str> for Directory {
    fn from(path: &str) -> Self {
        Directory::from(path.to_string())
    }
}

fn getent_group(container: &Container) -> Result<(HashSet<Groupname>, HashSet<u32>)> {
    let stdout = container
        .output(Command::new("getent").arg("group"))?
        .stdout()?;
    let mut groupnames = HashSet::new();
    let mut group_ids = HashSet::new();
    for line in stdout.lines() {
        let mut parts = line.split(':');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(name), Some(_), Some(id)) => {
                groupnames.insert(name.to_string());
                group_ids.insert(id.parse()?);
            }
            _ => {
                return Err(format!("invalid `getent group` syntax: {line}").into());
            }
        }
    }

    Ok((groupnames, group_ids))
}

fn getent_passwd(container: &Container) -> Result<(HashSet<Username>, HashSet<u32>)> {
    let stdout = container
        .output(Command::new("getent").arg("passwd"))?
        .stdout()?;
    let mut usernames = HashSet::new();
    let mut user_ids = HashSet::new();
    for line in stdout.lines() {
        let mut parts = line.split(':');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(name), Some(_), Some(id)) => {
                usernames.insert(name.to_string());
                user_ids.insert(id.parse()?);
            }
            _ => {
                return Err(format!("invalid `getent passwd` syntax: {line}").into());
            }
        }
    }

    Ok((usernames, user_ids))
}

#[cfg(test)]
mod tests {
    use super::*;

    const USERNAME: &str = "ferris";
    const GROUPNAME: &str = "rustaceans";

    #[test]
    fn group_creation_works() -> Result<()> {
        let env = EnvBuilder::default().group(GROUPNAME).build()?;

        let groupnames = getent_group(&env.container)?.0;
        assert!(groupnames.contains(GROUPNAME));

        Ok(())
    }

    #[test]
    fn user_creation_works() -> Result<()> {
        let env = EnvBuilder::default().user(USERNAME).build()?;

        let usernames = getent_passwd(&env.container)?.0;
        assert!(usernames.contains(USERNAME));

        Ok(())
    }

    #[test]
    fn no_implicit_home_creation() -> Result<()> {
        let env = EnvBuilder::default().user(USERNAME).build()?;

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("[ -d /home/{USERNAME} ]"))
            .output(&env)?;
        assert!(!output.status().success());
        Ok(())
    }

    #[test]
    fn no_implicit_user_group_creation() -> Result<()> {
        let env = EnvBuilder::default().user(USERNAME).build()?;

        let stdout = Command::new("groups")
            .as_user(USERNAME)
            .output(&env)?
            .stdout()?;
        let groups = stdout.split(' ').collect::<HashSet<_>>();
        assert!(!groups.contains(USERNAME));

        Ok(())
    }

    #[test]
    fn no_password_by_default() -> Result<()> {
        let env = EnvBuilder::default().user(USERNAME).build()?;

        let stdout = Command::new("passwd")
            .args(["--status", USERNAME])
            .output(&env)?
            .stdout()?;

        assert!(stdout.starts_with(&format!("{USERNAME} L")));

        Ok(())
    }

    #[test]
    fn password_assignment_works() -> Result<()> {
        let password = "strong-password";
        let env = Env("ALL ALL=(ALL:ALL) ALL")
            .user(User(USERNAME).password(password))
            .build()?;

        Command::new("sudo")
            .args(["-S", "true"])
            .as_user(USERNAME)
            .stdin(password)
            .output(&env)?
            .assert_success()
    }

    #[test]
    fn creating_user_part_of_existing_group_works() -> Result<()> {
        let groupname = "users";
        let env = EnvBuilder::default()
            .user(User(USERNAME).secondary_group(groupname))
            .build()?;

        let stdout = Command::new("groups")
            .as_user(USERNAME)
            .output(&env)?
            .stdout()?;
        let user_groups = stdout.split(' ').collect::<HashSet<_>>();
        assert!(user_groups.contains(groupname));

        Ok(())
    }

    #[test]
    fn sudoers_file_get_created_with_expected_contents() -> Result<()> {
        let expected = "Hello, root!";
        let env = Env(expected).build()?;

        let actual = Command::new("cat")
            .arg("/etc/sudoers")
            .output(&env)?
            .stdout()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn text_file_gets_created_with_right_perms() -> Result<()> {
        let chown = format!("{USERNAME}:{GROUPNAME}");
        let chmod = "600";
        let expected_contents = "hello";
        let path = "/root/file";
        let env = EnvBuilder::default()
            .user(USERNAME)
            .group(GROUPNAME)
            .file(path, TextFile(expected_contents).chown(chown).chmod(chmod))
            .build()?;

        let actual_contents = Command::new("cat").arg(path).output(&env)?.stdout()?;
        assert_eq!(expected_contents, &actual_contents);

        let ls_l = Command::new("ls")
            .args(["-l", path])
            .output(&env)?
            .stdout()?;
        assert!(ls_l.starts_with("-rw-------"));
        assert!(ls_l.contains(&format!("{USERNAME} {GROUPNAME}")));

        Ok(())
    }

    #[test]
    #[should_panic = "user root already exists in base image"]
    fn cannot_create_user_that_already_exists_in_base_image() {
        EnvBuilder::default().user("root").build().unwrap();
    }

    #[test]
    #[should_panic = "user ID 0 already exists in base image"]
    fn cannot_assign_user_id_that_already_exists_in_base_image() {
        EnvBuilder::default()
            .user(User(USERNAME).id(0))
            .build()
            .unwrap();
    }

    #[test]
    #[should_panic = "group root already exists in base image"]
    fn cannot_create_group_that_already_exists_in_base_image() {
        EnvBuilder::default().group("root").build().unwrap();
    }

    #[test]
    #[should_panic = "group ID 0 already exists in base image"]
    fn cannot_assign_group_id_that_already_exists_in_base_image() {
        EnvBuilder::default()
            .group(Group(GROUPNAME).id(0))
            .build()
            .unwrap();
    }

    #[test]
    fn setting_user_id_works() -> Result<()> {
        let expected = 1023;
        let env = EnvBuilder::default()
            .user(User(USERNAME).id(expected))
            .build()?;

        let actual = Command::new("id")
            .args(["-u", USERNAME])
            .output(&env)?
            .stdout()?
            .parse()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn setting_group_id_works() -> Result<()> {
        let expected = 1023;
        let env = EnvBuilder::default()
            .group(Group(GROUPNAME).id(expected))
            .build()?;

        let stdout = Command::new("getent")
            .args(["group", GROUPNAME])
            .output(&env)?
            .stdout()?;
        let actual = stdout.split(':').nth(2);
        assert_eq!(Some(expected.to_string().as_str()), actual);

        Ok(())
    }

    #[test]
    fn setting_hostname_works() -> Result<()> {
        let expected = "container";

        let env = EnvBuilder::default().hostname(expected).build()?;

        let actual = Command::new("hostname").output(&env)?.stdout()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn trailing_newline_by_default() -> Result<()> {
        let path_a = "/root/a";
        let path_b = "/root/b";
        let env = EnvBuilder::default()
            .file(path_a, "hello")
            .file(path_b, "hello\n")
            .build()?;

        let a_last_char = Command::new("tail")
            .args(["-c1", path_a])
            .output(&env)?
            .stdout()?;
        assert_eq!("", a_last_char);

        let b_last_char = Command::new("tail")
            .args(["-c1", path_b])
            .output(&env)?
            .stdout()?;
        assert_eq!("", b_last_char);

        Ok(())
    }

    #[test]
    fn no_trailing_newline() -> Result<()> {
        let path_a = "/root/a";
        let path_b = "/root/b";
        let env = EnvBuilder::default()
            .file(path_a, TextFile("hello").no_trailing_newline())
            .file(path_b, TextFile("hello\n").no_trailing_newline())
            .build()?;

        let a_last_char = Command::new("tail")
            .args(["-c1", path_a])
            .output(&env)?
            .stdout()?;
        assert_eq!("o", a_last_char);

        let b_last_char = Command::new("tail")
            .args(["-c1", path_b])
            .output(&env)?
            .stdout()?;

        assert_eq!("o", b_last_char);

        Ok(())
    }

    #[test]
    fn directory_gets_created_with_right_perms() -> Result<()> {
        let chown = format!("{USERNAME}:{GROUPNAME}");
        let chmod = "700";
        let path = "/tmp/dir";
        let env = EnvBuilder::default()
            .user(USERNAME)
            .group(GROUPNAME)
            .directory(Directory(path).chown(chown).chmod(chmod))
            .build()?;

        let ls_al = Command::new("ls")
            .args(["-al", path])
            .output(&env)?
            .stdout()?;
        let dot_entry = ls_al.lines().nth(1).unwrap();
        assert!(dot_entry.ends_with(" ."));
        assert!(dot_entry.starts_with("drwx------"));
        assert!(dot_entry.contains(&format!("{USERNAME} {GROUPNAME}")));

        Ok(())
    }

    #[test]
    #[should_panic = "mkdir: cannot create directory '/': File exists"]
    fn cannot_create_directory_that_already_exists() {
        EnvBuilder::default().directory("/").build().unwrap();
    }

    #[test]
    #[should_panic = "mkdir: cannot create directory '/root/a/b': No such file or directory"]
    fn cannot_create_directory_whose_parent_does_not_exist() {
        EnvBuilder::default()
            .directory("/root/a/b")
            .build()
            .unwrap();
    }

    #[test]
    fn can_create_file_in_declared_directory() -> Result<()> {
        let dir_path = "/root/dir";
        let file_path = "/root/dir/file";
        let env = EnvBuilder::default()
            .directory(dir_path)
            .file(file_path, "")
            .build()?;

        Command::new("sh")
            .arg("-c")
            .arg(format!("[ -d {dir_path} ]"))
            .output(&env)?
            .assert_success()?;

        Command::new("sh")
            .arg("-c")
            .arg(format!("[ -f {file_path} ]"))
            .output(&env)?
            .assert_success()?;

        Ok(())
    }

    #[test]
    fn run_as_nonexistent_user() -> Result<()> {
        let env = EnvBuilder::default().build()?;

        let output = Command::new("whoami").as_user_id(1000).output(&env)?;

        assert!(!output.status().success());
        assert_eq!("whoami: cannot find name for user ID 1000", output.stderr());

        Ok(())
    }

    #[test]
    fn create_home_directory_works() -> Result<()> {
        let env = EnvBuilder::default()
            .user(User(USERNAME).create_home_directory())
            .build()?;

        Command::new("sh")
            .arg("-c")
            .arg(format!("[ -d /home/{USERNAME} ]"))
            .output(&env)?
            .assert_success()
    }

    #[test]
    fn setting_shell_works() -> Result<()> {
        let expected = "/path/to/shell";
        let env = EnvBuilder::default()
            .user(User(USERNAME).shell(expected))
            .build()?;

        let passwd = Command::new("getent")
            .arg("passwd")
            .output(&env)?
            .stdout()?;

        let mut found = false;
        for line in passwd.lines() {
            if line.starts_with(&format!("{USERNAME}:")) {
                found = true;
                assert!(line.ends_with(&format!(":{expected}")));
            }
        }

        assert!(found);

        Ok(())
    }
}
