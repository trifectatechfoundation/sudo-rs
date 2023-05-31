use crate::cli::SuOptions;

mod cli;

fn main() {
    let su_options = SuOptions::from_env().unwrap();

    dbg!(su_options);
}
