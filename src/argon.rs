use argon2::{Algorithm::Argon2id, Argon2, Params, ParamsBuilder, PasswordHash, Version::V0x13};
use std::{fmt, sync::LazyLock};

use crate::S;

#[expect(clippy::unwrap_used)]
#[cfg(debug_assertions)]
static PARAMS: LazyLock<Params> = LazyLock::new(|| {
    ParamsBuilder::new()
        .m_cost(4096)
        .t_cost(1)
        .p_cost(1)
        .build()
        .unwrap()
});

#[expect(clippy::unwrap_used)]
#[cfg(not(debug_assertions))]
static PARAMS: LazyLock<Params> = LazyLock::new(|| {
    ParamsBuilder::new()
        .m_cost(24 * 1024)
        .t_cost(64)
        .p_cost(1)
        .build()
        .unwrap()
});

fn get_hasher() -> Argon2<'static> {
    Argon2::new(Argon2id, V0x13, PARAMS.clone())
}

// Need to look into this
#[derive(Clone, PartialEq, Eq)]
pub struct ArgonHash(String);

impl fmt::Debug for ArgonHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", "*".repeat(self.0.len()))
    }
}

#[cfg(test)]
impl fmt::Display for ArgonHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for ArgonHash {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if argon2::password_hash::PasswordHash::new(&value).is_ok() {
            Ok(Self(value))
        } else {
            Err(S!("argon hash invalid"))
        }
    }
}

impl ArgonHash {
    pub async fn verify_password(&self, password: &str) -> Result<bool, String> {
        let password = password.to_owned();
        let argon_hash = self.clone();
        tokio::task::spawn_blocking(move || -> Result<bool, String> {
            PasswordHash::new(&argon_hash.0).map_or(Err(S!("verify_password::new_hash")), |hash| {
                match hash.verify_password(&[&get_hasher()], password) {
                    Ok(()) => Ok(true),
                    Err(e) => match e {
                        // Could always just return false, no need to worry about internal errors?
                        argon2::password_hash::Error::Password => Ok(false),
                        _ => Err(S!("verify_password")),
                    },
                }
            })
        })
        .await
        .map_err(|_| S!("Join error"))?
    }
}

/// http tests - ran via actual requests to a (local) server
/// cargo watch -q -c -w src/ -x 'test argon_mod -- --test-threads=1 --nocapture'
#[cfg(test)]
#[expect(clippy::pedantic, clippy::unwrap_used)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn argon_mod_verify_known() {
        let password = "This is a known password";
        let password_hash = ArgonHash("$argon2id$v=19$m=4096,t=5,p=1$rahU5enqn3WcOo9A58Ifjw$I+7yA6+29LuB5jzPUwnxtLoH66Lng7ExWqHdivwj8Es".to_owned());

        // Verify true
        let result = password_hash.verify_password(password).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Verify false
        let result = password_hash
            .verify_password("this is a known password")
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
