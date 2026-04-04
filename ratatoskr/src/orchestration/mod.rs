use anyhow::{Context, anyhow};
use reqwest::Client;
use serde_json::json;

use crate::config::LifecycleAction;

#[derive(Clone)]
pub struct LifecycleExecutor {
    client: Client,
}

impl LifecycleExecutor {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn execute(&self, action: &LifecycleAction) -> anyhow::Result<()> {
        match action {
            LifecycleAction::NoAction => Ok(()),
            LifecycleAction::ReloadCaddy { admin_url } => self.reload_caddy(admin_url).await,
            LifecycleAction::RestartContainer {
                docker_proxy_url,
                container,
            } => self.restart_container(docker_proxy_url, container).await,
        }
    }

    async fn reload_caddy(&self, admin_url: &str) -> anyhow::Result<()> {
        let url = caddy_load_endpoint(admin_url);
        let response = self
            .client
            .post(url)
            .json(&json!({}))
            .send()
            .await
            .context("failed calling caddy admin API")?;
        if !response.status().is_success() {
            return Err(anyhow!("caddy reload failed with {}", response.status()));
        }
        Ok(())
    }

    async fn restart_container(
        &self,
        docker_proxy_url: &str,
        container: &str,
    ) -> anyhow::Result<()> {
        let url = restart_endpoint(docker_proxy_url, container);
        let response = self
            .client
            .post(url)
            .send()
            .await
            .context("failed calling docker proxy API")?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "docker restart request failed with {}",
                response.status()
            ));
        }
        Ok(())
    }
}

fn caddy_load_endpoint(admin_url: &str) -> String {
    format!("{}/load", admin_url.trim_end_matches('/'))
}

fn restart_endpoint(docker_proxy_url: &str, container: &str) -> String {
    format!(
        "{}/containers/{}/restart",
        docker_proxy_url.trim_end_matches('/'),
        container
    )
}

#[cfg(test)]
mod tests {
    use super::{caddy_load_endpoint, restart_endpoint};

    #[test]
    fn builds_caddy_reload_url() {
        assert_eq!(
            caddy_load_endpoint("http://caddy:2019/"),
            "http://caddy:2019/load"
        );
    }

    #[test]
    fn builds_restart_url() {
        assert_eq!(
            restart_endpoint("http://proxy:2375/", "authentik-server"),
            "http://proxy:2375/containers/authentik-server/restart"
        );
    }
}
