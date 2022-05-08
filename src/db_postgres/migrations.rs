use crate::api::AppError;
use sqlx::PgPool;

/// This will fail, as need to be owner of the database
/// and for postgresql security reasons, the db user that rust connects with is never the owner
/// but keeping in for future info
pub async fn migrations(_db: &PgPool) -> Result<(), AppError> {
    // let commands = include_str!("./migrations.sql");
    // let sql_commands = commands.split(';').collect::<Vec<_>>();
    // for command in sql_commands {
    //     match sqlx::query(command).execute(db).await {
    //         Ok(_) => (),
    //         Err(e) => println!(
    //             "migration failed: {}",
    //             e
    //         ),
    //     }
    // }
    Ok(())
}
