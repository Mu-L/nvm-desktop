use std::path::PathBuf;

use crate::{
    config::{Config, Project},
    log_err,
    utils::{dirs, help},
};
use anyhow::{anyhow, Result};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::DialogExt;

use super::handle;

/// get project list from `projects.json`
pub async fn project_list(fetch: Option<bool>) -> Result<Option<Vec<Project>>> {
    let fetch = fetch.unwrap_or(false);
    if !fetch {
        return Ok(Config::projects().latest().list.clone());
    }

    let path = dirs::projects_path()?;
    let list = help::async_read_json::<Vec<Project>>(&path).await?;

    // update projects
    Config::projects().latest().update_list(&list)?;
    Config::projects().apply();

    Ok(Some(list))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PInfo {
    /// project floder path
    pub path: PathBuf,

    /// project version from `.nvmdrc`
    pub version: Option<String>,
}

/// add projects
pub async fn add_projects(app_handle: tauri::AppHandle) -> Result<Option<Vec<PInfo>>> {
    let file_paths = app_handle.dialog().file().blocking_pick_folders();
    if file_paths.is_none() {
        return Ok(None);
    }

    let file_paths = file_paths.unwrap();
    let mut p_info = vec![];
    for file_path in file_paths {
        let nvmdrc_path = file_path.join(".nvmdrc");
        let version = if nvmdrc_path.exists() {
            Some(help::async_read_string(&nvmdrc_path).await?)
        } else {
            None
        };
        p_info.push(PInfo {
            path: file_path,
            version,
        });
    }

    Ok(Some(p_info))
}

/// update projects
pub async fn update_projects(list: Vec<Project>, path: Option<PathBuf>) -> Result<()> {
    if let Some(path) = path {
        tokio::fs::remove_file(&path.join(".nvmdrc")).await?;
    }

    Config::projects().apply();
    Config::projects().data().update_and_save_list(list)?;
    Ok(())
}

/// sync project version to `.nvmdrc`
pub async fn sync_project_version(path: PathBuf, version: &String) -> Result<i32> {
    if !path.exists() {
        return Ok(404);
    }

    let path = path.join(".nvmdrc");
    help::save_string(&path, version).await?;

    Ok(200)
}

/// batch update project version
pub async fn batch_update_project_version(paths: Vec<PathBuf>, version: String) -> Result<()> {
    let result = stream::iter(paths.into_iter())
        .map(|path| {
            let version = version.clone();
            async move {
                let path = path.join(".nvmdrc");
                help::save_string(&path, &version).await
            }
        })
        .buffer_unordered(3)
        .collect::<Vec<_>>()
        .await;

    for ret in result {
        ret?;
    }

    Ok(())
}

/// change project with version from menu
pub async fn change_with_version(name: String, version: String) -> Result<()> {
    let ret = {
        let project_path = Config::projects()
            .latest()
            .update_version(&name, &version)?;
        let need_update_groups = Config::groups().latest().update_projects(&project_path)?;

        sync_project_version(PathBuf::from(&project_path), &version).await?;

        log_err!(handle::Handle::update_systray_part(version));

        <Result<bool>>::Ok(need_update_groups)
    };

    match ret {
        Ok(need_update_groups) => {
            Config::projects().apply();
            Config::projects().data().save_file()?;

            if need_update_groups {
                Config::groups().apply();
                Config::groups().data().save_file()?;
            }

            Ok(())
        }
        Err(err) => {
            Config::projects().discard();
            Config::groups().discard();
            Err(err)
        }
    }
}

/// change project with group from menu
pub async fn change_with_group(name: String, group_name: String) -> Result<()> {
    let ret = {
        let project_path = Config::projects()
            .latest()
            .update_version(&name, &group_name)?;
        let version = Config::groups()
            .latest()
            .update_projects_version(&project_path, &group_name)?
            .ok_or_else(|| anyhow!("failed to find the group version \"name:{}\"", &group_name))?;

        sync_project_version(PathBuf::from(&project_path), &version).await?;

        log_err!(handle::Handle::update_systray_part(version));

        <Result<()>>::Ok(())
    };

    match ret {
        Ok(()) => {
            Config::projects().apply();
            Config::projects().data().save_file()?;

            Config::groups().apply();
            Config::groups().data().save_file()?;

            Ok(())
        }
        Err(err) => {
            Config::projects().discard();
            Config::groups().discard();
            Err(err)
        }
    }
}