use crate::env::get_enviroment;

mod env;

fn main() {
    let a = get_enviroment();
    println!("Environment: {:?}", a.into());
}