//! Install built-in workflow templates (Feature Closeout, etc.).

use api_types::{
    Squad, SquadOnAssign, SquadPipeline, SquadPipelineEdge, SquadPipelineNode,
    SquadPipelineNodeType, SquadTargetType,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    routing::post,
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_project_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{agents::AgentRepository, squads::SquadRepository},
};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/projects/{project_id}/workflow-templates/feature-closeout",
        post(install_feature_closeout),
    )
}

#[derive(Debug, serde::Serialize)]
pub struct InstallTemplateResponse {
    pub squad: Squad,
    pub agent_ids: Vec<Uuid>,
    pub created_agent_names: Vec<String>,
}

#[instrument(
    name = "workflow_templates.feature_closeout",
    skip(state, ctx),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn install_feature_closeout(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<InstallTemplateResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    let mut created_names = Vec::new();
    let mut agent_ids = Vec::new();

    let reviewer = ensure_agent(
        state.pool(),
        project_id,
        "Closeout Reviewer",
        "你是代码 Reviewer。对照 Issue 验收标准检查 diff；列出缺口；不够则明确写「需回修」与具体条目。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(reviewer);

    let tester = ensure_agent(
        state.pool(),
        project_id,
        "Closeout Tester",
        "你是测试员。按新功能 / 旧核心 / 交叉三项 checklist 验证，并写清命令与结果。不过则标明失败项。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(tester);

    let fixer = ensure_agent(
        state.pool(),
        project_id,
        "Closeout Fixer",
        "你是修复员。只修 Reviewer/Tester 指出的点，禁止扩 scope。修完后简要说明改动。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(fixer);

    let n_review = "n_review".to_string();
    let n_enough = "n_enough".to_string();
    let n_fix = "n_fix".to_string();
    let n_test = "n_test".to_string();
    let n_script = "n_script".to_string();
    let n_rebase = "n_rebase".to_string();
    let n_ask = "n_ask".to_string();

    let pipeline = SquadPipeline {
        nodes: vec![
            SquadPipelineNode {
                id: n_review.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(reviewer),
                role: Some("reviewer".into()),
                prompt: Some(
                    "Review 当前 Issue 关联 workspace 的 diff，对照验收标准输出缺口清单。"
                        .into(),
                ),
                label: Some("Review".into()),
                entry_label: Some("代码审查".into()),
                stage: Some("verify".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_enough.clone(),
                node_type: SquadPipelineNodeType::If,
                condition: Some("agent:需回修".into()),
                label: Some("完成度?".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_fix.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(fixer),
                role: Some("fixer".into()),
                prompt: Some("根据上一轮 Review/Test 缺口回修，不要扩需求。".into()),
                label: Some("回修".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_test.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(tester),
                role: Some("tester".into()),
                prompt: Some(
                    "执行新功能/旧核心/交叉验证；失败写明原因。成功则总结测试报告。".into(),
                ),
                label: Some("测试设计".into()),
                entry_label: Some("测试验证".into()),
                stage: Some("verify".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_script.clone(),
                node_type: SquadPipelineNodeType::Script,
                command: Some("pnpm run check".into()),
                label: Some("跑 check".into()),
                entry_label: Some("跑检查脚本".into()),
                stage: Some("verify".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_rebase.clone(),
                node_type: SquadPipelineNodeType::GitOp,
                git_op: Some("rebase".into()),
                wait_for: Some("main".into()),
                label: Some("Rebase".into()),
                entry_label: Some("Rebase 主干".into()),
                stage: Some("merge".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_ask.clone(),
                node_type: SquadPipelineNodeType::WaitApproval,
                approval_kind: Some("merge".into()),
                prompt_template: Some(
                    "【Ask Merge】Review/测试/rebase 已完成。是否合并到主干？\nApprove = 继续合并流程；Reject = 停止。"
                        .into(),
                ),
                label: Some("Ask Merge".into()),
                entry_label: Some("Ask Merge".into()),
                stage: Some("merge".into()),
                ..Default::default()
            },
        ],
        edges: vec![
            edge("e1", &n_review, &n_enough, None),
            edge(
                "e2",
                &n_enough,
                &n_fix,
                Some(api_types::SquadPipelineEdgeBranch::True),
            ),
            edge(
                "e3",
                &n_enough,
                &n_test,
                Some(api_types::SquadPipelineEdgeBranch::False),
            ),
            edge("e4", &n_fix, &n_review, None),
            edge("e5", &n_test, &n_script, None),
            edge("e6", &n_script, &n_rebase, None),
            edge("e7", &n_rebase, &n_ask, None),
        ],
        loop_config: None,
    };

    // Upsert squad by name
    let existing = SquadRepository::list_by_project(state.pool(), project_id)
        .await
        .map_err(|e| db_error(e, "list squads"))?;
    let squad = if let Some(s) = existing.into_iter().find(|s| s.name == "Feature Closeout") {
        SquadRepository::update(
            state.pool(),
            s.id,
            Some("Feature Closeout".into()),
            Some(Some(reviewer)),
            Some(pipeline),
            Some(SquadTargetType::IssueAndPath),
            None,
            None,
            Some(SquadOnAssign::FullPipeline),
        )
        .await
        .map_err(|e| db_error(e, "update closeout squad"))?
        .data
    } else {
        SquadRepository::create(
            state.pool(),
            None,
            project_id,
            "Feature Closeout".into(),
            Some(reviewer),
            Some(pipeline),
            SquadTargetType::IssueAndPath,
            None,
            None,
            SquadOnAssign::FullPipeline,
        )
        .await
        .map_err(|e| db_error(e, "create closeout squad"))?
        .data
    };

    Ok(Json(InstallTemplateResponse {
        squad,
        agent_ids,
        created_agent_names: created_names,
    }))
}

fn edge(
    id: &str,
    source: &str,
    target: &str,
    branch: Option<api_types::SquadPipelineEdgeBranch>,
) -> SquadPipelineEdge {
    SquadPipelineEdge {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        branch,
    }
}

async fn ensure_agent(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    name: &str,
    instructions: &str,
    created: &mut Vec<String>,
) -> Result<Uuid, ErrorResponse> {
    let agents = AgentRepository::list_by_project(pool, project_id)
        .await
        .map_err(|e| db_error(e, "list agents"))?;
    if let Some(a) = agents.into_iter().find(|a| a.name == name) {
        return Ok(a.id);
    }
    let resp = AgentRepository::create(
        pool,
        None,
        project_id,
        name.into(),
        instructions.into(),
        None,
        2,
        api_types::AgentChatRuntime::Cursor,
        None,
    )
    .await
    .map_err(|e| db_error(e, "create agent"))?;
    created.push(name.into());
    Ok(resp.data.id)
}
