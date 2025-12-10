pub enum Environment {
    Development,
    Production,
}

impl Into<&'static str> for Environment {
    fn into(self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Production => "production",
        }
    }
}

impl Into<Environment> for &str {
    fn into(self) -> Environment {
        match self {
            "production" => Environment::Production,
            _ => Environment::Development,
        }
    }
}

pub fn get_enviroment() -> Environment {
    std::env::var("PROFILE").unwrap_or_default().as_str().into()
}