use crate::env::get_enviroment;

mod env;

fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_default();
    println!("Profile: {}", profile);
    get_enviroment();
}