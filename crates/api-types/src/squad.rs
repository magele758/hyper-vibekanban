use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Squad {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub leader_agent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, sqlx::FromRow)]
pub struct SquadMember {
    pub id: Uuid,
    pub squad_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateSquadRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub name: String,
    #[ts(optional)]
    pub leader_agent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct UpdateSquadRequest {
    #[serde(default, deserialize_with = "some_if_present")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub leader_agent_id: Option<Option<Uuid>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListSquadsQuery {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListSquadsResponse {
    pub squads: Vec<Squad>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AddSquadMemberRequest {
    pub squad_id: Uuid,
    #[ts(optional)]
    pub agent_id: Option<Uuid>,
    #[ts(optional)]
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListSquadMembersResponse {
    pub members: Vec<SquadMember>,
}
