pub mod auth;
pub mod db;
pub mod handlers;
pub mod models;
pub mod ui;

pub fn build_router(db: db::Db) -> axum::Router {
    handlers::build_router(db)
}

pub fn handle_cli_command(db: &db::Db, args: &[String]) -> Result<Option<String>, String> {
    if args.len() >= 2 && args[1] == "create-token" {
        let mut user_opt = None;
        let mut name_opt = None;
        let mut i = 2;
        while i < args.len() {
            if args[i] == "--user" && i + 1 < args.len() {
                user_opt = Some(args[i + 1].clone());
                i += 2;
            } else if args[i] == "--name" && i + 1 < args.len() {
                name_opt = Some(args[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        }
        let username = user_opt.ok_or_else(|| "Missing required flag --user <username>".to_string())?;
        let token_name = name_opt.unwrap_or_else(|| "CLI Token".to_string());

        let user = db
            .get_user_by_username(&username)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("User '{}' not found", username))?;

        if user.access_level < 25 {
            return Err(format!(
                "User '{}' has access level {}, but Reporter (25) or higher is required",
                username, user.access_level
            ));
        }

        let (_id, raw_token) = db.create_api_token(user.id, &token_name)?;
        return Ok(Some(format!(
            "Created API token '{}' for user '{}':\n{}",
            token_name, username, raw_token
        )));
    }
    Ok(None)
}
