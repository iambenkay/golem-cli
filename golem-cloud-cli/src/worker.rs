// Copyright 2024 Golem Cloud
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use async_trait::async_trait;
use clap::builder::ValueParser;
use clap::Subcommand;
use golem_cloud_client::model::{InvokeParameters, WorkerMetadata, WorkersMetadataResponse};
use uuid::Uuid;

use crate::clients::worker::WorkerClient;
use crate::component::ComponentHandler;
use crate::model::{
    ComponentIdOrName, GolemError, GolemResult, IdempotencyKey, JsonValueParser, WorkerName,
    WorkerUpdateMode,
};
use crate::parse_key_val;

#[derive(Subcommand, Debug)]
#[command()]
pub enum WorkerSubcommand {
    /// Creates a new idle worker
    #[command()]
    Add {
        /// The Golem componen to use for the worker, identified by either its name or its componen ID
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the newly created worker
        #[arg(short, long)]
        worker_name: WorkerName,

        /// List of environment variables (key-value pairs) passed to the worker
        #[arg(short, long, value_parser = parse_key_val)]
        env: Vec<(String, String)>,

        /// List of command line arguments passed to the worker
        #[arg(value_name = "args")]
        args: Vec<String>,
    },

    /// Generates an idempotency ID for achieving at-most-one invocation when doing retries
    #[command()]
    IdempotencyKey {},

    /// Invokes a worker and waits for its completion
    #[command()]
    InvokeAndAwait {
        /// The Golem componen the worker to be invoked belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker
        #[arg(short, long)]
        worker_name: WorkerName,

        /// A pre-generated idempotency key, if not provided, a new one will be generated
        #[arg(short = 'k', long)]
        idempotency_key: Option<IdempotencyKey>,

        /// Name of the function to be invoked
        #[arg(short, long)]
        function: String,

        /// JSON array representing the parameters to be passed to the function
        #[arg(short = 'j', long, value_name = "json", value_parser = ValueParser::new(JsonValueParser))]
        parameters: serde_json::value::Value,

        /// Enables the STDIO calling convention, passing the parameters through stdin instead of a typed exported interface
        #[arg(short = 's', long, default_value_t = false)]
        use_stdio: bool,
    },

    /// Triggers a function invocation on a worker without waiting for its completion
    #[command()]
    Invoke {
        /// The Golem componen the worker to be invoked belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker
        #[arg(short, long)]
        worker_name: WorkerName,

        /// A pre-generated idempotency key
        #[arg(short = 'k', long)]
        idempotency_key: Option<IdempotencyKey>,

        /// Name of the function to be invoked
        #[arg(short, long)]
        function: String,

        /// JSON array representing the parameters to be passed to the function
        #[arg(short = 'j', long, value_name = "json", value_parser = ValueParser::new(JsonValueParser))]
        parameters: serde_json::value::Value,
    },

    /// Connect to a worker and live stream its standard output, error and log channels
    #[command()]
    Connect {
        /// The Golem componen the worker to be connected to belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker
        #[arg(short, long)]
        worker_name: WorkerName,
    },

    /// Interrupts a running worker
    #[command()]
    Interrupt {
        /// The Golem componen the worker to be interrupted belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker
        #[arg(short, long)]
        worker_name: WorkerName,
    },

    /// Simulates a crash on a worker for testing purposes.
    ///
    /// The worker starts recovering and resuming immediately.
    #[command()]
    SimulatedCrash {
        /// The Golem componen the worker to be crashed belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker
        #[arg(short, long)]
        worker_name: WorkerName,
    },

    /// Deletes a worker
    #[command()]
    Delete {
        /// The Golem componen the worker to be deleted belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker
        #[arg(short, long)]
        worker_name: WorkerName,
    },

    /// Retrieves metadata about an existing worker
    #[command()]
    Get {
        /// The Golem componen the worker to be retrieved belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker
        #[arg(short, long)]
        worker_name: WorkerName,
    },
    /// Retrieves metadata about an existing workers in a component
    #[command()]
    List {
        /// The Golem componen the workers to be retrieved belongs to
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Filter for worker metadata in form of `property op value`.
        ///
        /// Filter examples: `name = worker-name`, `version >= 0`, `status = Running`, `env.var1 = value`.
        /// Can be used multiple times (AND condition is applied between them)
        #[arg(short, long)]
        filter: Option<Vec<String>>,

        /// Position where to start listing, if not provided, starts from the beginning
        ///
        /// It is used to get the next page of results. To get next page, use the cursor returned in the response
        #[arg(short, long)]
        cursor: Option<u64>,

        /// Count of listed values, if count is not provided, returns all values
        #[arg(short = 'n', long)]
        count: Option<u64>,

        /// Precision in relation to worker status, if true, calculate the most up-to-date status for each worker, default is false
        #[arg(short, long)]
        precise: Option<bool>,
    },
    /// Updates a worker
    #[command()]
    Update {
        /// The Golem component of the worker, identified by either its name or its component ID
        #[command(flatten)]
        component_id_or_name: ComponentIdOrName,

        /// Name of the worker to update
        #[arg(short, long)]
        worker_name: WorkerName,

        /// Update mode - auto or manual
        #[arg(short, long)]
        mode: WorkerUpdateMode,

        /// The new version of the updated worker
        #[arg(short = 't', long)]
        target_version: u64,
    },
}

#[async_trait]
pub trait WorkerHandler {
    async fn handle(&self, subcommand: WorkerSubcommand) -> Result<GolemResult, GolemError>;
}

pub struct WorkerHandlerLive<'r, C: WorkerClient + Send + Sync, R: ComponentHandler + Send + Sync> {
    pub client: C,
    pub components: &'r R,
}

#[async_trait]
impl<'r, C: WorkerClient + Send + Sync, R: ComponentHandler + Send + Sync> WorkerHandler
    for WorkerHandlerLive<'r, C, R>
{
    async fn handle(&self, subcommand: WorkerSubcommand) -> Result<GolemResult, GolemError> {
        match subcommand {
            WorkerSubcommand::Add {
                component_id_or_name,
                worker_name,
                env,
                args,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                let inst = self
                    .client
                    .new_worker(worker_name, component_id, args, env)
                    .await?;

                Ok(GolemResult::Ok(Box::new(inst)))
            }
            WorkerSubcommand::IdempotencyKey {} => {
                let key = IdempotencyKey(Uuid::new_v4().to_string());

                Ok(GolemResult::Ok(Box::new(key)))
            }
            WorkerSubcommand::InvokeAndAwait {
                component_id_or_name,
                worker_name,
                idempotency_key,
                function,
                parameters,
                use_stdio,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                let res = self
                    .client
                    .invoke_and_await(
                        worker_name,
                        component_id,
                        function,
                        InvokeParameters { params: parameters },
                        idempotency_key,
                        use_stdio,
                    )
                    .await?;

                Ok(GolemResult::Json(res.result))
            }
            WorkerSubcommand::Invoke {
                component_id_or_name,
                worker_name,
                idempotency_key,
                function,
                parameters,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                self.client
                    .invoke(
                        worker_name,
                        component_id,
                        function,
                        InvokeParameters { params: parameters },
                        idempotency_key,
                    )
                    .await?;

                Ok(GolemResult::Str("Invoked".to_string()))
            }
            WorkerSubcommand::Connect {
                component_id_or_name,
                worker_name,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                let result = self.client.connect(worker_name, component_id).await;

                match result {
                    Ok(_) => Err(GolemError("Unexpected connection closure".to_string())),
                    Err(err) => Err(GolemError(err.to_string())),
                }
            }
            WorkerSubcommand::Interrupt {
                component_id_or_name,
                worker_name,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                self.client.interrupt(worker_name, component_id).await?;

                Ok(GolemResult::Str("Interrupted".to_string()))
            }
            WorkerSubcommand::SimulatedCrash {
                component_id_or_name,
                worker_name,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                self.client
                    .simulated_crash(worker_name, component_id)
                    .await?;

                Ok(GolemResult::Str("Done".to_string()))
            }
            WorkerSubcommand::Delete {
                component_id_or_name,
                worker_name,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                self.client.delete(worker_name, component_id).await?;

                Ok(GolemResult::Str("Deleted".to_string()))
            }
            WorkerSubcommand::Get {
                component_id_or_name,
                worker_name,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                let mata = self.client.get_metadata(worker_name, component_id).await?;

                Ok(GolemResult::Ok(Box::new(mata)))
            }
            WorkerSubcommand::List {
                component_id_or_name,
                filter,
                count,
                cursor,
                precise,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;

                if count.is_some() {
                    let response = self
                        .client
                        .list_metadata(component_id, filter, cursor, count, precise)
                        .await?;

                    Ok(GolemResult::Ok(Box::new(response)))
                } else {
                    let mut workers: Vec<WorkerMetadata> = vec![];
                    let mut new_cursor = cursor;

                    loop {
                        let response = self
                            .client
                            .list_metadata(
                                component_id.clone(),
                                filter.clone(),
                                new_cursor,
                                Some(50),
                                precise,
                            )
                            .await?;

                        workers.extend(response.workers);

                        new_cursor = response.cursor;

                        if new_cursor.is_none() {
                            break;
                        }
                    }

                    Ok(GolemResult::Ok(Box::new(WorkersMetadataResponse {
                        workers,
                        cursor: None,
                    })))
                }
            }
            WorkerSubcommand::Update {
                component_id_or_name,
                worker_name,
                target_version,
                mode,
            } => {
                let component_id = self.components.resolve_id(component_id_or_name).await?;
                let _ = self
                    .client
                    .update(worker_name, component_id, mode, target_version)
                    .await?;

                Ok(GolemResult::Str("Updated".to_string()))
            }
        }
    }
}
