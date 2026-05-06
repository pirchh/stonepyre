use sqlx::PgPool;
use uuid::Uuid;

use crate::game::protocol::{SkillDelta, SkillSnapshot, SkillSnapshotEntry};

pub const WOODCUTTING_SKILL_ID: &str = "woodcutting";
pub const WOODCUTTING_DISPLAY_NAME: &str = "Woodcutting";

const MAX_SKILL_LEVEL: u32 = 99;

#[derive(Clone, Debug)]
pub struct SkillXpGrantRequest {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub skill_id: String,
    pub display_name: String,
    pub xp_delta: u32,
    pub action: crate::game::protocol::InteractionAction,
    pub target: crate::game::protocol::InteractionTarget,
    pub node_id: String,
    pub harvest_display_name: String,
    pub charges_remaining: u32,
}

#[derive(Clone, Debug)]
pub struct SkillXpGrantResult {
    pub character_id: Uuid,
    pub skill_id: String,
    pub display_name: String,
    pub xp_delta: i64,
    pub new_xp: i64,
    pub new_level: u32,
    pub xp_for_next_level: Option<i64>,
}

#[derive(Clone, Copy, Debug)]
pub struct SkillProgress {
    pub xp: i64,
    pub level: u32,
    pub xp_for_next_level: Option<i64>,
}


pub async fn load_character_skill_progress(
    pool: &PgPool,
    character_id: Uuid,
    skill_id: &str,
) -> Result<SkillProgress, sqlx::Error> {
    let xp = load_character_skill_xp(pool, character_id, skill_id).await?;
    Ok(skill_progress_for_xp(xp))
}

pub async fn load_character_skill_xp(
    pool: &PgPool,
    character_id: Uuid,
    skill_id: &str,
) -> Result<i64, sqlx::Error> {
    let xp: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT xp
        FROM game.character_skills
        WHERE character_id = $1::uuid
          AND skill_id = $2::text
        "#,
    )
    .bind(character_id)
    .bind(skill_id)
    .fetch_optional(pool)
    .await?;

    Ok(xp.unwrap_or(0))
}

pub async fn load_character_skills_snapshot(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<SkillSnapshot, sqlx::Error> {
    let woodcutting_xp = load_character_skill_xp(pool, character_id, WOODCUTTING_SKILL_ID).await?;
    let progress = skill_progress_for_xp(woodcutting_xp);

    Ok(SkillSnapshot {
        character_id,
        skills: vec![SkillSnapshotEntry {
            skill_id: WOODCUTTING_SKILL_ID.to_string(),
            display_name: WOODCUTTING_DISPLAY_NAME.to_string(),
            xp: progress.xp,
            level: progress.level,
            xp_for_next_level: progress.xp_for_next_level,
        }],
    })
}

pub async fn grant_character_skill_xp(
    pool: &PgPool,
    character_id: Uuid,
    skill_id: &str,
    display_name: &str,
    xp_delta: u32,
) -> Result<SkillXpGrantResult, sqlx::Error> {
    let xp_delta_i64 = i64::from(xp_delta);

    let new_xp: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO game.character_skills (character_id, skill_id, xp)
        VALUES ($1::uuid, $2::text, $3::bigint)
        ON CONFLICT (character_id, skill_id)
        DO UPDATE SET
            xp = game.character_skills.xp + EXCLUDED.xp,
            updated_at = now()
        RETURNING xp
        "#,
    )
    .bind(character_id)
    .bind(skill_id)
    .bind(xp_delta_i64)
    .fetch_one(pool)
    .await?;

    let progress = skill_progress_for_xp(new_xp);

    Ok(SkillXpGrantResult {
        character_id,
        skill_id: skill_id.to_string(),
        display_name: display_name.to_string(),
        xp_delta: xp_delta_i64,
        new_xp: progress.xp,
        new_level: progress.level,
        xp_for_next_level: progress.xp_for_next_level,
    })
}

pub fn skill_delta_from_result(result: SkillXpGrantResult) -> SkillDelta {
    SkillDelta {
        character_id: result.character_id,
        skill_id: result.skill_id,
        display_name: result.display_name,
        xp_delta: result.xp_delta,
        new_xp: result.new_xp,
        new_level: result.new_level,
        xp_for_next_level: result.xp_for_next_level,
    }
}

pub fn skill_progress_for_xp(xp: i64) -> SkillProgress {
    let safe_xp = xp.max(0);
    let level = level_for_xp(safe_xp);
    SkillProgress {
        xp: safe_xp,
        level,
        xp_for_next_level: if level >= MAX_SKILL_LEVEL {
            None
        } else {
            Some(xp_for_level(level + 1))
        },
    }
}

/// OSRS-style XP curve:
/// level 1 starts at 0 XP, and each next level is derived from the familiar
/// cumulative points formula. Keeping this in one helper makes it easy to swap
/// the curve later if Stonepyre needs a different progression pace.
pub fn xp_for_level(level: u32) -> i64 {
    let clamped = level.clamp(1, MAX_SKILL_LEVEL);
    if clamped <= 1 {
        return 0;
    }

    let mut points = 0.0_f64;

    for lvl in 1..clamped {
        points += f64::from(lvl) + 300.0 * 2.0_f64.powf(f64::from(lvl) / 7.0);
    }

    (points / 4.0).floor() as i64
}

pub fn level_for_xp(xp: i64) -> u32 {
    let safe_xp = xp.max(0);

    let mut level = 1;
    for candidate in 2..=MAX_SKILL_LEVEL {
        if safe_xp >= xp_for_level(candidate) {
            level = candidate;
        } else {
            break;
        }
    }

    level
}
