//! 세션별 generating/finished 상태 추적 + 트레이 아이콘 렌더 트리거.
//!
//! 규칙:
//!  - generating → idle 전환된 세션을 `finished` 에 추가 (set_at = now)
//!  - 해제 트리거: row 클릭, tray 클릭, idle→generating 재전환, 프로세스 종료,
//!    24시간 경과
//!  - running snapshot 에 없는 세션은 "프로세스 종료" 로 간주 → 제거

use crate::parser::Activity;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, SystemTime},
};

const FINISHED_EXPIRY_SECS: u64 = 60 * 60 * 24; // 24h
const GENERATING_WINDOW_SECS: u64 = 60 * 60 * 12; // 12h mtime 윈도우

#[derive(Debug, Default)]
pub struct TrayState {
    /// 직전 tick 에 generating 이었던 세션 ID.
    prev_generating: HashSet<String>,
    /// 확인 대기 중인 완료 세션 → set_at 시각.
    finished: HashMap<String, SystemTime>,
}

/// poll 결과 → 트레이 반영용 요약.
#[derive(Debug, Clone, Serialize)]
pub struct TraySummary {
    pub generating_count: usize,
    pub finished_count: usize,
    pub generating_ids: Vec<String>,
    pub finished_ids: Vec<String>,
}

/// poll 에서 전달받는 세션 관측치 — (session_id, running_activity_option, mtime_fresh)
/// running_activity_option = None 이면 그 세션 프로세스 없음.
pub struct SessionObservation {
    pub id: String,
    pub activity: Option<Activity>,
    pub mtime_fresh: bool,
}

impl TrayState {
    /// 현재 세션 관측치로 상태를 갱신하고 tray summary 를 반환.
    pub fn update(&mut self, observations: &[SessionObservation]) -> TraySummary {
        let mut current_generating: HashSet<String> = HashSet::new();
        let running_ids: HashSet<&str> = observations.iter().map(|o| o.id.as_str()).collect();

        for obs in observations {
            match obs.activity {
                Some(Activity::Generating) if obs.mtime_fresh => {
                    current_generating.insert(obs.id.clone());
                    // 유저가 다시 메시지 보냄 → finished 해제
                    self.finished.remove(&obs.id);
                }
                _ => {}
            }
        }

        // prev_generating - current_generating 이고 지금 idle 상태 → finished 편입
        for id in self.prev_generating.difference(&current_generating) {
            if !running_ids.contains(id.as_str()) {
                // running snapshot 에 아예 없음 = 프로세스 종료 — finished 에 넣지 않음
                continue;
            }
            self.finished.entry(id.clone()).or_insert_with(SystemTime::now);
        }

        // running 에서 사라진 세션의 finished 제거 (프로세스 종료 해제 규칙)
        self.finished.retain(|id, _| running_ids.contains(id.as_str()));

        // 24h 경과한 finished 만료
        let now = SystemTime::now();
        self.finished.retain(|_, set_at| {
            now.duration_since(*set_at)
                .map(|d| d < Duration::from_secs(FINISHED_EXPIRY_SECS))
                .unwrap_or(false)
        });

        self.prev_generating = current_generating.clone();

        let mut generating_ids: Vec<String> = current_generating.into_iter().collect();
        generating_ids.sort();
        let mut finished_ids: Vec<String> = self.finished.keys().cloned().collect();
        finished_ids.sort();

        TraySummary {
            generating_count: generating_ids.len(),
            finished_count: finished_ids.len(),
            generating_ids,
            finished_ids,
        }
    }

    pub fn acknowledge(&mut self, session_id: &str) {
        self.finished.remove(session_id);
    }

    pub fn acknowledge_all(&mut self) {
        self.finished.clear();
    }

    pub fn is_finished(&self, session_id: &str) -> bool {
        self.finished.contains_key(session_id)
    }
}

/// 앱 전역 상태. Mutex 로 보호.
pub struct SharedTrayState(pub Mutex<TrayState>);

impl SharedTrayState {
    pub fn new() -> Self {
        Self(Mutex::new(TrayState::default()))
    }
}

pub fn is_mtime_fresh(path: &std::path::Path) -> bool {
    let Ok(md) = std::fs::metadata(path) else { return false };
    let Ok(modified) = md.modified() else { return false };
    SystemTime::now()
        .duration_since(modified)
        .map(|d| d.as_secs() < GENERATING_WINDOW_SECS)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: &str, act: Option<Activity>) -> SessionObservation {
        SessionObservation {
            id: id.to_string(),
            activity: act,
            mtime_fresh: true,
        }
    }

    #[test]
    fn transitions_generating_to_finished() {
        let mut s = TrayState::default();
        let sum = s.update(&[obs("a", Some(Activity::Generating))]);
        assert_eq!(sum.generating_count, 1);
        assert_eq!(sum.finished_count, 0);

        let sum = s.update(&[obs("a", Some(Activity::Idle))]);
        assert_eq!(sum.generating_count, 0);
        assert_eq!(sum.finished_count, 1);
    }

    #[test]
    fn retransition_clears_finished() {
        let mut s = TrayState::default();
        s.update(&[obs("a", Some(Activity::Generating))]);
        s.update(&[obs("a", Some(Activity::Idle))]);
        assert!(s.is_finished("a"));
        // 다시 generating 으로 → 해제
        let sum = s.update(&[obs("a", Some(Activity::Generating))]);
        assert_eq!(sum.finished_count, 0);
        assert!(!s.is_finished("a"));
    }

    #[test]
    fn process_gone_clears_finished() {
        let mut s = TrayState::default();
        s.update(&[obs("a", Some(Activity::Generating))]);
        s.update(&[obs("a", Some(Activity::Idle))]);
        assert!(s.is_finished("a"));
        // running snapshot 에 사라짐
        let sum = s.update(&[]);
        assert_eq!(sum.finished_count, 0);
    }

    #[test]
    fn acknowledge_removes() {
        let mut s = TrayState::default();
        s.update(&[obs("a", Some(Activity::Generating))]);
        s.update(&[obs("a", Some(Activity::Idle))]);
        s.acknowledge("a");
        assert!(!s.is_finished("a"));
    }
}
