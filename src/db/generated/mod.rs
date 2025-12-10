// Auto-generated module declarations
// This file is generated - do not edit manually
// Run `halvor db generate` to regenerate

pub mod settings;
pub mod encrypted_env_data;
pub mod host_info;
pub mod smb_servers;
pub mod update_history;


// Re-export all generated structs
pub use settings::SettingsRow;
pub use settings::SettingsRowData;
pub use settings::{insert_one, insert_many, upsert_one, select_one, select_many, delete_by_id};
pub use encrypted_env_data::EncryptedEnvDataRow;
pub use encrypted_env_data::EncryptedEnvDataRowData;
pub use encrypted_env_data::{insert_one, insert_many, upsert_one, select_one, select_many, delete_by_id};
pub use host_info::HostInfoRow;
pub use host_info::HostInfoRowData;
pub use host_info::{insert_one, insert_many, upsert_one, select_one, select_many, delete_by_id};
pub use smb_servers::SmbServersRow;
pub use smb_servers::SmbServersRowData;
pub use smb_servers::{insert_one, insert_many, upsert_one, select_one, select_many, delete_by_id};
pub use update_history::UpdateHistoryRow;
pub use update_history::UpdateHistoryRowData;
pub use update_history::{insert_one, insert_many, upsert_one, select_one, select_many, delete_by_id};

