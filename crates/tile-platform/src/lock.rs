//! Taking over Win+L.
//!
//! Windows handles Win+L below any keyboard hook, so the only way to use it as a
//! keybind is the per-user `DisableLockWorkstation` policy. While it is set, locking
//! goes through [`lock_workstation`], which lifts the policy just long enough to lock.

use winsafe::{self as w, co};

const POLICY_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Policies\System";
const POLICY_VALUE: &str = "DisableLockWorkstation";

/// Whether locking is currently disabled by the per-user policy.
pub fn lock_disabled() -> bool {
    matches!(
        w::HKEY::CURRENT_USER.RegGetValue(Some(POLICY_KEY), Some(POLICY_VALUE), co::RRF::RT_REG_DWORD),
        Ok(w::RegistryValue::Dword(v)) if v != 0
    )
}

/// Disables (`true`) or re-enables (`false`) Win+L and workstation locking.
/// Returns whether it worked; the policy key is admin-only on some machines.
pub fn set_lock_disabled(disabled: bool) -> bool {
    let result = if disabled {
        w::HKEY::CURRENT_USER.RegSetKeyValue(
            Some(POLICY_KEY),
            Some(POLICY_VALUE),
            w::RegistryValue::Dword(1),
        )
    } else {
        w::HKEY::CURRENT_USER
            .RegOpenKeyEx(
                Some(POLICY_KEY),
                co::REG_OPTION::default(),
                co::KEY::SET_VALUE,
            )
            .and_then(|key| key.RegDeleteValue(Some(POLICY_VALUE)))
            .or_else(|e| {
                if e == co::ERROR::FILE_NOT_FOUND {
                    Ok(())
                } else {
                    Err(e)
                }
            })
    };
    if let Err(e) = &result {
        tracing::debug!(
            "failed to {} Win+L: {e}",
            if disabled { "take over" } else { "restore" }
        );
    }
    result.is_ok()
}

/// PowerShell that asks for elevation once (UAC) and lets the current user manage the
/// policy key. Only `QueryValues` and `SetValue` on that one key are granted.
pub const GRANT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$inner = @"
`$p = 'Registry::HKEY_USERS\$sid\Software\Microsoft\Windows\CurrentVersion\Policies\System'
if (-not (Test-Path `$p)) { New-Item -Path `$p -Force | Out-Null }
`$acl = Get-Acl `$p
`$id = New-Object Security.Principal.SecurityIdentifier('$sid')
`$rule = New-Object Security.AccessControl.RegistryAccessRule(`$id, 'QueryValues, SetValue', 'Allow')
`$acl.AddAccessRule(`$rule)
Set-Acl -Path `$p -AclObject `$acl
"@
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($inner))
Start-Process powershell -Verb RunAs -WindowStyle Hidden -ArgumentList '-NoProfile', '-EncodedCommand', $encoded
"#;

/// Locks the workstation, temporarily lifting our policy if needed.
/// Returns whether the policy was lifted, so the caller can re-apply it shortly after.
pub fn lock_workstation() -> bool {
    let lifted = lock_disabled() && set_lock_disabled(false);
    if let Err(e) = w::LockWorkStation() {
        tracing::warn!("LockWorkStation failed: {e}");
    }
    lifted
}
