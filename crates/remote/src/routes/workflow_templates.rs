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
    Router::new()
        .route(
            "/projects/{project_id}/workflow-templates/feature-closeout",
            post(install_feature_closeout),
        )
        .route(
            "/projects/{project_id}/workflow-templates/feature-full-flow",
            post(install_feature_full_flow),
        )
        .route(
            "/projects/{project_id}/workflow-templates/relentless-delivery",
            post(install_relentless_delivery),
        )
        .route(
            "/projects/{project_id}/workflow-templates/scout-digest",
            post(install_scout_digest),
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

    // Hardened Closeout (while = loop-until-true, max 5):
    //
    //   Review ──► while(until verified:验收通过) ──exit──► Test → script → rebase → Ask → merge
    //                 │ body
    //                 └─► Fix ──► Re-review  (no edge back to while; the for-loop rechecks)
    //
    // script error ──► Fix ──► Review ──► while (re-enter after hard-check failure)
    let n_review = "n_review".to_string();
    let n_loop = "n_loop".to_string();
    let n_fix = "n_fix".to_string();
    let n_rereview = "n_rereview".to_string();
    let n_test = "n_test".to_string();
    let n_script = "n_script".to_string();
    let n_fix_script = "n_fix_script".to_string();
    let n_rebase = "n_rebase".to_string();
    let n_ask = "n_ask".to_string();
    let n_merge = "n_merge".to_string();

    let review_prompt = "Review 当前 Issue 关联 workspace 的 **真实 diff**（见下方 handoff），\
         对照验收标准输出缺口清单。全部满足则明确写「验收通过」；\
         否则写「需回修」并列出条目。";

    let pipeline = SquadPipeline {
        nodes: vec![
            SquadPipelineNode {
                id: n_review.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(reviewer),
                role: Some("reviewer".into()),
                prompt: Some(review_prompt.into()),
                label: Some("Review".into()),
                entry_label: Some("代码审查".into()),
                stage: Some("verify".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_loop.clone(),
                node_type: SquadPipelineNodeType::While,
                // Loop until reviewer writes 验收通过 (strict: step must be Completed).
                condition: Some("verified:验收通过".into()),
                max_iterations: Some(5),
                label: Some("直到审查通过".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_fix.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(fixer),
                role: Some("fixer".into()),
                prompt: Some("根据上一轮 Review 缺口回修，不要扩需求。修完后简要说明。".into()),
                label: Some("回修".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            // Body-only re-review: no outgoing edge so the while for-loop rechecks
            // `last_agent_result` instead of nesting another while entry.
            SquadPipelineNode {
                id: n_rereview.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(reviewer),
                role: Some("reviewer".into()),
                prompt: Some(review_prompt.into()),
                label: Some("再审查".into()),
                stage: Some("verify".into()),
                handoff_diff: Some(true),
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
                handoff_diff: Some(true),
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
            // Script-failure recovery (outside the review while).
            SquadPipelineNode {
                id: n_fix_script.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(fixer),
                role: Some("fixer".into()),
                prompt: Some(
                    "检查脚本失败了。根据 stdout/stderr 与测试反馈修到能通过，不要扩需求。".into(),
                ),
                label: Some("脚本失败回修".into()),
                handoff_diff: Some(true),
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
                    "【Ask Merge】Review/测试/rebase 已完成。是否合并到主干？\n\
                     Approve = 执行 workspace merge；Reject = 停止。"
                        .into(),
                ),
                label: Some("Ask Merge".into()),
                entry_label: Some("Ask Merge".into()),
                stage: Some("merge".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_merge.clone(),
                node_type: SquadPipelineNodeType::GitOp,
                git_op: Some("merge".into()),
                wait_for: Some("main".into()),
                label: Some("Merge".into()),
                entry_label: Some("合并主干".into()),
                stage: Some("merge".into()),
                ..Default::default()
            },
        ],
        edges: vec![
            edge("e1", &n_review, &n_loop, None),
            // Body: not yet accepted → fix → re-review (terminal in body).
            edge(
                "e2",
                &n_loop,
                &n_fix,
                Some(api_types::SquadPipelineEdgeBranch::Body),
            ),
            edge("e3", &n_fix, &n_rereview, None),
            // Exit: acceptance written → test → hard check → rebase → gate → merge.
            edge(
                "e4",
                &n_loop,
                &n_test,
                Some(api_types::SquadPipelineEdgeBranch::Exit),
            ),
            edge("e5", &n_test, &n_script, None),
            edge("e6", &n_script, &n_rebase, None),
            // Script red → dedicated fixer → re-enter Review→while.
            edge(
                "e7",
                &n_script,
                &n_fix_script,
                Some(api_types::SquadPipelineEdgeBranch::Error),
            ),
            edge("e7b", &n_fix_script, &n_review, None),
            edge("e8", &n_rebase, &n_ask, None),
            edge("e9", &n_ask, &n_merge, None),
        ],
        loop_config: None,
    };

    let squad = upsert_squad(&state, project_id, "Feature Closeout", reviewer, pipeline).await?;

    Ok(Json(InstallTemplateResponse {
        squad,
        agent_ids,
        created_agent_names: created_names,
    }))
}

/// Full Feature Flow — idea → scheme gate → design gate → implement → closeout tail.
///
/// Front half is conversation/docs oriented; after implement it reuses the same
/// review / script / rebase / Ask Merge / merge pattern as Feature Closeout.
#[instrument(
    name = "workflow_templates.feature_full_flow",
    skip(state, ctx),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn install_feature_full_flow(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<InstallTemplateResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    let mut created_names = Vec::new();
    let mut agent_ids = Vec::new();

    let researcher = ensure_agent(
        state.pool(),
        project_id,
        "Flow Researcher",
        "你是需求与方案研究员。把 Issue 收敛成：目标、非目标、验收标准、可选方案。\
         本步禁止改业务代码。输出清晰的 proposal，结尾写「方案待确认」。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(researcher);

    let architect = ensure_agent(
        state.pool(),
        project_id,
        "Flow Architect",
        "你是架构师。在已确认方案基础上给出模块边界、影响范围（API/DB/前端）、选型结论。\
         本步禁止大范围写业务实现。输出 design + impact，结尾写「选型待确认」。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(architect);

    let implementer = ensure_agent(
        state.pool(),
        project_id,
        "Flow Implementer",
        "你是实现者。严格按已确认的方案与架构实现完整需求，自检验收项。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(implementer);

    let reviewer = ensure_agent(
        state.pool(),
        project_id,
        "Closeout Reviewer",
        "你是代码 Reviewer。对照 Issue 验收标准检查 diff；列出缺口；不够则明确写「需回修」与具体条目。全部满足写「验收通过」。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(reviewer);

    let fixer = ensure_agent(
        state.pool(),
        project_id,
        "Closeout Fixer",
        "你是修复员。只修 Reviewer/Tester/脚本指出的点，禁止扩 scope。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(fixer);

    let n_research = "n_research".to_string();
    let n_scheme = "n_scheme".to_string();
    let n_design = "n_design".to_string();
    let n_design_gate = "n_design_gate".to_string();
    let n_impl = "n_impl".to_string();
    let n_review = "n_review".to_string();
    let n_loop = "n_loop".to_string();
    let n_fix = "n_fix".to_string();
    let n_rereview = "n_rereview".to_string();
    let n_script = "n_script".to_string();
    let n_fix_script = "n_fix_script".to_string();
    let n_rebase = "n_rebase".to_string();
    let n_ask = "n_ask".to_string();
    let n_merge = "n_merge".to_string();

    let review_prompt = "Review **真实 diff**（handoff）。对照验收标准；全部满足写「验收通过」，\
         否则写「需回修」并列条目。";

    let pipeline = SquadPipeline {
        nodes: vec![
            SquadPipelineNode {
                id: n_research.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(researcher),
                role: Some("researcher".into()),
                prompt: Some(
                    "针对本 Issue 产出 proposal：目标 / 非目标 / 验收标准 / 可选方案。不要改业务代码。"
                        .into(),
                ),
                label: Some("研究方案".into()),
                entry_label: Some("采集研究".into()),
                stage: Some("ideate".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_scheme.clone(),
                node_type: SquadPipelineNodeType::WaitApproval,
                approval_kind: Some("scheme".into()),
                prompt_template: Some(
                    "【确认方案】上方是研究产出的 proposal。\nApprove = 进入架构；Reject = 停止。"
                        .into(),
                ),
                label: Some("确认方案".into()),
                entry_label: Some("确认方案".into()),
                stage: Some("ideate".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_design.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(architect),
                role: Some("architect".into()),
                prompt: Some(
                    "方案已确认。产出 design + impact（模块边界、API/DB/前端影响、选型）。不要大改业务代码。"
                        .into(),
                ),
                label: Some("架构设计".into()),
                entry_label: Some("架构设计".into()),
                stage: Some("design".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_design_gate.clone(),
                node_type: SquadPipelineNodeType::WaitApproval,
                approval_kind: Some("design".into()),
                prompt_template: Some(
                    "【选型确认】上方是架构与影响范围。\nApprove = 开始实现；Reject = 停止。"
                        .into(),
                ),
                label: Some("选型确认".into()),
                entry_label: Some("选型确认".into()),
                stage: Some("design".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_impl.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(implementer),
                role: Some("implementer".into()),
                prompt: Some("按已确认方案与架构完整实现本 Issue。".into()),
                label: Some("实现".into()),
                entry_label: Some("开始实现".into()),
                stage: Some("implement_full".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_review.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(reviewer),
                role: Some("reviewer".into()),
                prompt: Some(review_prompt.into()),
                label: Some("Review".into()),
                entry_label: Some("代码审查".into()),
                stage: Some("verify".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_loop.clone(),
                node_type: SquadPipelineNodeType::While,
                condition: Some("verified:验收通过".into()),
                max_iterations: Some(5),
                label: Some("直到审查通过".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_fix.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(fixer),
                role: Some("fixer".into()),
                prompt: Some("按审查缺口回修，禁止扩 scope。".into()),
                label: Some("回修".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_rereview.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(reviewer),
                role: Some("reviewer".into()),
                prompt: Some(review_prompt.into()),
                label: Some("再审查".into()),
                stage: Some("verify".into()),
                handoff_diff: Some(true),
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
                id: n_fix_script.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(fixer),
                role: Some("fixer".into()),
                prompt: Some("检查脚本失败，按日志修复至通过。".into()),
                label: Some("脚本失败回修".into()),
                handoff_diff: Some(true),
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
                    "【Ask Merge】实现与审查/检查/rebase 已完成。是否合并？\n\
                     Approve = merge；Reject = 停止。"
                        .into(),
                ),
                label: Some("Ask Merge".into()),
                entry_label: Some("Ask Merge".into()),
                stage: Some("merge".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_merge.clone(),
                node_type: SquadPipelineNodeType::GitOp,
                git_op: Some("merge".into()),
                wait_for: Some("main".into()),
                label: Some("Merge".into()),
                entry_label: Some("合并主干".into()),
                stage: Some("merge".into()),
                ..Default::default()
            },
        ],
        edges: vec![
            edge("f1", &n_research, &n_scheme, None),
            edge("f2", &n_scheme, &n_design, None),
            edge("f3", &n_design, &n_design_gate, None),
            edge("f4", &n_design_gate, &n_impl, None),
            edge("f5", &n_impl, &n_review, None),
            edge("f6", &n_review, &n_loop, None),
            edge(
                "f7",
                &n_loop,
                &n_fix,
                Some(api_types::SquadPipelineEdgeBranch::Body),
            ),
            edge("f8", &n_fix, &n_rereview, None),
            edge(
                "f9",
                &n_loop,
                &n_script,
                Some(api_types::SquadPipelineEdgeBranch::Exit),
            ),
            edge("f10", &n_script, &n_rebase, None),
            edge(
                "f11",
                &n_script,
                &n_fix_script,
                Some(api_types::SquadPipelineEdgeBranch::Error),
            ),
            edge("f12", &n_fix_script, &n_review, None),
            edge("f13", &n_rebase, &n_ask, None),
            edge("f14", &n_ask, &n_merge, None),
        ],
        loop_config: None,
    };

    let squad = upsert_squad(
        &state,
        project_id,
        "Full Feature Flow",
        researcher,
        pipeline,
    )
    .await?;

    Ok(Json(InstallTemplateResponse {
        squad,
        agent_ids,
        created_agent_names: created_names,
    }))
}

/// Relentless Delivery — keep pushing a coding agent until the work is
/// *verifiably* done.
///
/// Coding agents habitually stop half-way and declare success. This template
/// refuses to take their word for it: the loop can only exit when a real script
/// (tests / typecheck) exits 0, and `max_iterations` guarantees it terminates.
#[instrument(
    name = "workflow_templates.relentless_delivery",
    skip(state, ctx),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn install_relentless_delivery(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<InstallTemplateResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    let mut created_names = Vec::new();
    let mut agent_ids = Vec::new();

    let builder = ensure_agent(
        state.pool(),
        project_id,
        "Delivery Builder",
        "你负责把 Issue 实现到「可验证完成」。禁止交付半成品：\
         每轮结束前自己先跑项目的检查/测试命令。若还有未完成项，明确列出剩余 TODO。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(builder);

    let auditor = ensure_agent(
        state.pool(),
        project_id,
        "Completion Auditor",
        "你是完成度审计员。对照 Issue 验收标准逐条核对实际 diff 与测试输出。\
         只有全部满足才写「验收通过」；否则写「需回修」并列出具体缺口，不要客套。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(auditor);

    let n_build = "n_build".to_string();
    let n_loop = "n_loop".to_string();
    let n_verify = "n_verify".to_string();
    let n_audit = "n_audit".to_string();
    let n_need_fix = "n_need_fix".to_string();
    let n_push = "n_push".to_string();
    let n_done = "n_done".to_string();

    let pipeline = SquadPipeline {
        nodes: vec![
            SquadPipelineNode {
                id: n_build.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(builder),
                role: Some("builder".into()),
                prompt: Some(
                    "实现本 Issue 的完整需求。完成后自检：改动是否覆盖所有验收项。".into(),
                ),
                label: Some("实现".into()),
                entry_label: Some("开始实现".into()),
                stage: Some("implement_full".into()),
                // Reviewer/auditor downstream should judge code, not prose.
                handoff_diff: Some(true),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_loop.clone(),
                node_type: SquadPipelineNodeType::While,
                // Until-true semantics: body runs while marker is missing; exit
                // when last agent result is Completed AND contains 验收通过.
                condition: Some("verified:验收通过".into()),
                max_iterations: Some(5),
                label: Some("直到验收通过".into()),
                entry_label: Some("进入督办循环".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_verify.clone(),
                node_type: SquadPipelineNodeType::Script,
                // Hard gate: a non-zero exit marks the step Failed, which the
                // strict condition treats as "not done" no matter what the
                // agent claimed.
                command: Some("pnpm run check".into()),
                label: Some("硬验证(check)".into()),
                entry_label: Some("跑检查".into()),
                stage: Some("verify".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_audit.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(auditor),
                role: Some("auditor".into()),
                prompt: Some(
                    "对照验收标准核对 diff 与检查结果。全部满足写「验收通过」，\
                     否则写「需回修」并列出缺口。"
                        .into(),
                ),
                label: Some("完成度审计".into()),
                entry_label: Some("审计完成度".into()),
                stage: Some("verify".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            // Only run fixer when auditor asked for rework — otherwise body ends
            // on audit so `verified:验收通过` can satisfy the while head next iter.
            SquadPipelineNode {
                id: n_need_fix.clone(),
                node_type: SquadPipelineNodeType::If,
                condition: Some("agent:需回修".into()),
                label: Some("需回修?".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_push.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(builder),
                role: Some("builder".into()),
                prompt: Some("上一步审计给出了未完成项。只补齐这些缺口，不要扩需求。".into()),
                label: Some("鞭策回修".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_done.clone(),
                node_type: SquadPipelineNodeType::WaitApproval,
                approval_kind: Some("merge".into()),
                prompt_template: Some(
                    "【验收通过】检查脚本已过、审计确认完成。是否合并？\n\
                     Approve = 继续；Reject = 停止。"
                        .into(),
                ),
                label: Some("Ask Merge".into()),
                entry_label: Some("Ask Merge".into()),
                stage: Some("merge".into()),
                ..Default::default()
            },
        ],
        edges: vec![
            edge("e1", &n_build, &n_loop, None),
            // Loop body: verify → audit → (only if 需回修) fix.
            // When audit writes 验收通过, if has no true edge taken; body ends
            // with last=audit so the while head exits on the next check.
            edge(
                "e2",
                &n_loop,
                &n_verify,
                Some(api_types::SquadPipelineEdgeBranch::Body),
            ),
            edge("e3", &n_verify, &n_audit, None),
            // Verification itself failing still needs an audit pass to explain why.
            edge(
                "e4",
                &n_verify,
                &n_audit,
                Some(api_types::SquadPipelineEdgeBranch::Error),
            ),
            edge("e5", &n_audit, &n_need_fix, None),
            edge(
                "e6",
                &n_need_fix,
                &n_push,
                Some(api_types::SquadPipelineEdgeBranch::True),
            ),
            edge(
                "e7",
                &n_loop,
                &n_done,
                Some(api_types::SquadPipelineEdgeBranch::Exit),
            ),
        ],
        loop_config: None,
    };

    let squad = upsert_squad(&state, project_id, "Relentless Delivery", builder, pipeline).await?;

    Ok(Json(InstallTemplateResponse {
        squad,
        agent_ids,
        created_agent_names: created_names,
    }))
}

/// Scout Digest — gather information on a schedule, ask the human, then build.
///
/// Pair this squad with an Autopilot cron. The collector runs a shell script
/// (curl / rss / gh api — whatever the repo provides), the analyst judges
/// relevance for *this* project, and `wait_approval` guarantees nothing is built
/// until you say so. Rejecting the proposal ends the run.
#[instrument(
    name = "workflow_templates.scout_digest",
    skip(state, ctx),
    fields(project_id = %project_id, user_id = %ctx.user.id)
)]
async fn install_scout_digest(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<InstallTemplateResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, project_id).await?;

    let mut created_names = Vec::new();
    let mut agent_ids = Vec::new();

    let analyst = ensure_agent(
        state.pool(),
        project_id,
        "Scout Analyst",
        "你是情报分析师。输入是采集到的原始信息。结合本仓库的技术栈与现状，\
         判断哪些值得做成特性。输出：发现了什么 / 对本项目的价值 / 建议改动范围 / 优先级。\
         没有价值就直说没有，不要凑数。禁止在本步修改任何代码。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(analyst);

    let implementer = ensure_agent(
        state.pool(),
        project_id,
        "Scout Implementer",
        "你根据已获批准的提案实现特性。严格限定在提案范围内，完成后跑项目检查命令。",
        &mut created_names,
    )
    .await?;
    agent_ids.push(implementer);

    let n_collect = "n_collect".to_string();
    let n_analyze = "n_analyze".to_string();
    let n_ask = "n_ask".to_string();
    let n_build = "n_build".to_string();
    let n_check = "n_check".to_string();

    let pipeline = SquadPipeline {
        nodes: vec![
            SquadPipelineNode {
                id: n_collect.clone(),
                node_type: SquadPipelineNodeType::Script,
                // Placeholder: point this at whatever the repo uses to fetch
                // news (curl an RSS feed, `gh api`, a scraper script...).
                // Edit it in the pipeline editor after installing.
                command: Some(
                    "echo '请将本命令改为真实的信息采集命令，例如 curl 某个 RSS / gh api / 自建脚本'"
                        .into(),
                ),
                label: Some("采集信息".into()),
                entry_label: Some("采集信息".into()),
                stage: Some("ideate".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_analyze.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(analyst),
                role: Some("analyst".into()),
                prompt: Some(
                    "上一步是采集到的原始信息。判断对本项目的价值并给出提案；\
                     本步不要改代码。"
                        .into(),
                ),
                label: Some("价值分析".into()),
                entry_label: Some("分析价值".into()),
                stage: Some("ideate".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_ask.clone(),
                node_type: SquadPipelineNodeType::WaitApproval,
                approval_kind: Some("scheme".into()),
                prompt_template: Some(
                    "【发现了新情报】上方是分析结果与建议提案。\n\
                     Approve = 立即开干；Reject = 丢弃这条情报。"
                        .into(),
                ),
                label: Some("询问是否开干".into()),
                entry_label: Some("等我拍板".into()),
                stage: Some("ideate".into()),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_build.clone(),
                node_type: SquadPipelineNodeType::Agent,
                agent_id: Some(implementer),
                role: Some("implementer".into()),
                prompt: Some("提案已获批准，按提案范围实现。".into()),
                label: Some("实现特性".into()),
                stage: Some("implement_full".into()),
                handoff_diff: Some(true),
                ..Default::default()
            },
            SquadPipelineNode {
                id: n_check.clone(),
                node_type: SquadPipelineNodeType::Script,
                command: Some("pnpm run check".into()),
                label: Some("跑检查".into()),
                stage: Some("verify".into()),
                ..Default::default()
            },
        ],
        edges: vec![
            edge("e1", &n_collect, &n_analyze, None),
            edge("e2", &n_analyze, &n_ask, None),
            edge("e3", &n_ask, &n_build, None),
            edge("e4", &n_build, &n_check, None),
        ],
        loop_config: None,
    };

    let squad = upsert_squad(&state, project_id, "Scout Digest", analyst, pipeline).await?;

    Ok(Json(InstallTemplateResponse {
        squad,
        agent_ids,
        created_agent_names: created_names,
    }))
}

/// Create or update a squad by name, so installing a template twice is safe.
async fn upsert_squad(
    state: &AppState,
    project_id: Uuid,
    name: &str,
    leader: Uuid,
    pipeline: SquadPipeline,
) -> Result<Squad, ErrorResponse> {
    let existing = SquadRepository::list_by_project(state.pool(), project_id)
        .await
        .map_err(|e| db_error(e, "list squads"))?;

    let squad = if let Some(s) = existing.into_iter().find(|s| s.name == name) {
        SquadRepository::update(
            state.pool(),
            s.id,
            Some(name.to_string()),
            Some(Some(leader)),
            Some(pipeline),
            Some(SquadTargetType::IssueAndPath),
            None,
            None,
            Some(SquadOnAssign::FullPipeline),
        )
        .await
        .map_err(|e| db_error(e, "update squad"))?
        .data
    } else {
        SquadRepository::create(
            state.pool(),
            None,
            project_id,
            name.to_string(),
            Some(leader),
            Some(pipeline),
            SquadTargetType::IssueAndPath,
            None,
            None,
            SquadOnAssign::FullPipeline,
        )
        .await
        .map_err(|e| db_error(e, "create squad"))?
        .data
    };

    Ok(squad)
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
        None,
    )
    .await
    .map_err(|e| db_error(e, "create agent"))?;
    created.push(name.into());
    Ok(resp.data.id)
}
