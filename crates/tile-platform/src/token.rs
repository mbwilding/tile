//! Who this process runs as.

use winsafe::{self as w, co};

/// Whether this process runs elevated (as administrator).
pub fn is_elevated() -> bool {
    matches!(
        w::HACCESSTOKEN::GetCurrentProcessToken().GetTokenInformation(co::TOKEN_INFORMATION_CLASS::Elevation),
        Ok(w::TokenInfo::Elevation(elevation)) if elevation.TokenIsElevated()
    )
}

/// The SID of the user this process runs as, e.g. `S-1-5-21-…`.
pub fn user_sid() -> Option<String> {
    let token = w::HACCESSTOKEN::GetCurrentProcessToken();
    let Ok(w::TokenInfo::User(user)) = token.GetTokenInformation(co::TOKEN_INFORMATION_CLASS::User) else {
        return None;
    };
    user.User.Sid().and_then(|sid| w::ConvertSidToStringSid(sid).ok())
}
