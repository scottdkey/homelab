use crate::backup;
use crate::config;
use anyhow::Result;

pub enum BackupCommand {
    Create,
    List,
    Restore { backup: Option<String> },
}

pub fn handle_backup(hostname: &str, command: BackupCommand) -> Result<()> {
    let homelab_dir = config::find_homelab_dir()?;
    let config = config::load_env_config(&homelab_dir)?;
    match command {
        BackupCommand::Create => backup::backup_host(hostname, &config)?,
        BackupCommand::List => backup::list_backups(hostname, &config)?,
        BackupCommand::Restore { backup } => {
            backup::restore_host(hostname, backup.as_deref(), &config)?
        }
    }
    Ok(())
}
