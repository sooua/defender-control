use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use windows::Win32::Globalization::GetUserDefaultUILanguage;

#[derive(Clone, Copy, PartialEq)]
pub enum Lang {
    En,   // English
    Zh,   // Chinese (Simplified)
    Ja,   // Japanese
    Ko,   // Korean
}

// -1 = auto-detect; 0=En 1=Zh 2=Ja 3=Ko = user override.
static FORCED_LANG: AtomicI32 = AtomicI32::new(-1);

fn lang_idx(l: Lang) -> i32 {
    match l {
        Lang::En => 0,
        Lang::Zh => 1,
        Lang::Ja => 2,
        Lang::Ko => 3,
    }
}
fn idx_lang(i: i32) -> Lang {
    match i {
        1 => Lang::Zh,
        2 => Lang::Ja,
        3 => Lang::Ko,
        _ => Lang::En,
    }
}
fn lang_code(l: Lang) -> &'static str {
    match l {
        Lang::En => "en",
        Lang::Zh => "zh",
        Lang::Ja => "ja",
        Lang::Ko => "ko",
    }
}
fn code_lang(s: &str) -> Option<Lang> {
    match s.trim() {
        "en" => Some(Lang::En),
        "zh" => Some(Lang::Zh),
        "ja" => Some(Lang::Ja),
        "ko" => Some(Lang::Ko),
        _ => None,
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("DefenderControl").join("lang.txt")
}

/// Load the persisted language override (call once at startup).
pub fn init() {
    if let Ok(s) = std::fs::read_to_string(config_path()) {
        if let Some(l) = code_lang(&s) {
            FORCED_LANG.store(lang_idx(l), Ordering::Relaxed);
        }
    }
}

/// The effective language: user override if set, otherwise OS auto-detect.
pub fn current_lang() -> Lang {
    let f = FORCED_LANG.load(Ordering::Relaxed);
    if f >= 0 {
        idx_lang(f)
    } else {
        detect_lang()
    }
}

/// Set and persist the user's language choice.
pub fn set_lang(l: Lang) {
    FORCED_LANG.store(lang_idx(l), Ordering::Relaxed);
    let p = config_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, lang_code(l));
}

pub struct Strings {
    pub lang: Lang,
    pub app_title: &'static str,
    pub status_group: &'static str,
    pub status_label: &'static str,
    pub realtime_label: &'static str,
    pub antivirus_label: &'static str,
    pub tamper_label: &'static str,
    pub btn_disable: &'static str,
    pub btn_enable: &'static str,
    pub btn_refresh: &'static str,
    pub btn_watch_install: &'static str,
    pub btn_watch_remove: &'static str,
    pub service_group: &'static str,
    pub log_group: &'static str,
    pub col_name: &'static str,
    pub col_status: &'static str,
    pub col_startup: &'static str,
    pub status_disabled: &'static str,
    pub status_protected: &'static str,
    pub status_partial: &'static str,
    pub status_on: &'static str,
    pub status_off: &'static str,
    pub status_unknown: &'static str,
    pub log_ti_acquire: &'static str,
    pub log_restore_point: &'static str,
    pub log_disable_tamper: &'static str,
    pub log_kill_smartscreen: &'static str,
    pub log_stop_services: &'static str,
    pub log_strip_ppl: &'static str,
    pub log_disable_drivers: &'static str,
    pub log_write_registry: &'static str,
    pub log_mp_pref: &'static str,
    pub log_set_ifeo: &'static str,
    pub log_install_watch: &'static str,
    pub log_enable_tamper: &'static str,
    pub log_restore_ppl: &'static str,
    pub log_start_services: &'static str,
    pub log_enable_drivers: &'static str,
    pub log_remove_ifeo: &'static str,
    pub log_remove_watch: &'static str,
    pub log_restore_altitude: &'static str,
    pub msg_disable_ok: &'static str,
    pub msg_enable_ok: &'static str,
    pub msg_ti_fail: &'static str,
    pub msg_op_fail: &'static str,
    pub msg_op_panic: &'static str,
    pub msg_watch_removed: &'static str,
    pub msg_watch_installed: &'static str,
    pub error_title: &'static str,
    pub confirm_title: &'static str,
    pub confirm_disable: &'static str,
    pub log_backup: &'static str,
    pub log_restore_backup: &'static str,
    pub startup_fail: &'static str,
    pub btn_min: &'static str,
    pub btn_close: &'static str,
    pub btn_copy: &'static str,
    pub btn_clear: &'static str,
    pub btn_more: &'static str,
    pub restore_point_desc: &'static str,
    pub msg_tamper_title: &'static str,
    pub msg_tamper_body: &'static str,
}

fn detect_lang() -> Lang {
    unsafe {
        let lid = GetUserDefaultUILanguage();
        let primary = lid & 0x3FF;
        let sub = (lid >> 10) & 0x3F;
        match (primary, sub) {
            (4, _) => Lang::Zh,    // Chinese (Simplified)
            (17, _) => Lang::Ja,    // Japanese
            (18, _) => Lang::Ko,    // Korean
            _ => Lang::En,          // Everything else -> English
        }
    }
}

pub fn get_strings() -> Strings {
    match current_lang() {
        Lang::Zh => Strings {
            lang: Lang::Zh,
            app_title: "Defender Control",
            status_group: "状态",
            status_label: "状态",
            realtime_label: "实时",
            antivirus_label: "反病毒",
            tamper_label: "篡改",
            btn_disable: "关闭 Defender",
            btn_enable: "开启 Defender",
            btn_refresh: "刷新状态",
            btn_watch_install: "安装防恢复计划任务",
            btn_watch_remove: "移除防恢复计划任务",
            service_group: "服务状态",
            log_group: "日志",
            col_name: "服务名称",
            col_status: "状态",
            col_startup: "启动类型",
            status_disabled: "已关闭",
            status_protected: "已保护",
            status_partial: "部分开启",
            status_on: "开启",
            status_off: "关闭",
            status_unknown: "--",
            log_ti_acquire: "正在获取 TrustedInstaller 权限...",
            log_restore_point: "正在创建系统还原点...",
            log_disable_tamper: "正在关闭篡改防护...",
            log_kill_smartscreen: "正在终止 SmartScreen...",
            log_stop_services: "正在停止 Defender 服务...",
            log_strip_ppl: "正在剥离服务 PPL 保护标志...",
            log_disable_drivers: "正在禁用 Defender 内核驱动 (Altitude)...",
            log_write_registry: "正在写入注册表配置...",
            log_mp_pref: "正在配置 MpPreference (PowerShell)...",
            log_set_ifeo: "正在设置 IFEO 拦截 mpcmdrun.exe...",
            log_install_watch: "正在安装防恢复计划任务...",
            log_enable_tamper: "正在恢复篡改防护...",
            log_restore_ppl: "正在恢复服务 PPL 保护...",
            log_start_services: "正在恢复 Defender 服务...",
            log_enable_drivers: "正在恢复 Defender 驱动...",
            log_remove_ifeo: "正在移除 IFEO 拦截...",
            log_remove_watch: "正在移除防恢复计划任务...",
            log_restore_altitude: "正在恢复 WdFilter Altitude...",
            msg_disable_ok: "已成功关闭，建议重启系统",
            msg_enable_ok: "已成功开启，建议重启系统",
            msg_ti_fail: "权限提升失败: ",
            msg_op_fail: "操作失败: ",
            msg_op_panic: "操作异常终止",
            msg_watch_removed: "已移除防恢复计划任务",
            msg_watch_installed: "防恢复计划任务已安装（开机自动检查）",
            error_title: "错误",
            confirm_title: "确认操作",
            confirm_disable: "确定要关闭 Windows Defender 吗？\n这会降低系统安全性。当前配置会自动备份，可随时恢复。",
            log_backup: "正在备份当前配置...",
            log_restore_backup: "正在从备份恢复原始配置...",
            startup_fail: "启动失败",
            btn_min: "—",
            btn_close: "✕",
            btn_copy: "复制",
            btn_clear: "清空",
            btn_more: "更多",
            restore_point_desc: "Defender Control - 系统配置更改",
            msg_tamper_title: "需要先关闭篡改防护",
            msg_tamper_body: "Windows 安全中心的「篡改防护」已开启,它会自动撤销本工具所做的更改,导致无法关闭 Defender。\n\n请打开:Windows 安全中心 → 病毒和威胁防护 → 管理设置,关闭「篡改防护」,然后重试。",
        },
        Lang::Ja => Strings {
            lang: Lang::Ja,
            app_title: "Defender Control",
            status_group: "状態",
            status_label: "状態",
            realtime_label: "リアルタイム",
            antivirus_label: "ウイルス",
            tamper_label: "改ざん",
            btn_disable: "Defender を無効化",
            btn_enable: "Defender を有効化",
            btn_refresh: "更新",
            btn_watch_install: "復元防止タスクをインストール",
            btn_watch_remove: "復元防止タスクを削除",
            service_group: "サービス状態",
            log_group: "ログ",
            col_name: "サービス名",
            col_status: "状態",
            col_startup: "開始タイプ",
            status_disabled: "無効",
            status_protected: "保護中",
            status_partial: "一部有効",
            status_on: "有効",
            status_off: "無効",
            status_unknown: "--",
            log_ti_acquire: "TrustedInstaller 権限を取得中...",
            log_restore_point: "システム復元ポイントを作成中...",
            log_disable_tamper: "改ざん防止を無効化中...",
            log_kill_smartscreen: "SmartScreen を終了中...",
            log_stop_services: "Defender サービスを停止中...",
            log_strip_ppl: "サービス PPL 保護を除去中...",
            log_disable_drivers: "Defender カーネルドライバを無効化中...",
            log_write_registry: "レジストリ設定を書き込み中...",
            log_mp_pref: "MpPreference を設定中...",
            log_set_ifeo: "IFEO で mpcmdrun.exe をブロック中...",
            log_install_watch: "復元防止タスクをインストール中...",
            log_enable_tamper: "改ざん防止を復元中...",
            log_restore_ppl: "サービス PPL 保護を復元中...",
            log_start_services: "Defender サービスを復元中...",
            log_enable_drivers: "Defender ドライバを復元中...",
            log_remove_ifeo: "IFEO ブロックを解除中...",
            log_remove_watch: "復元防止タスクを削除中...",
            log_restore_altitude: "WdFilter Altitude を復元中...",
            msg_disable_ok: "無効化しました。再起動を推奨します。",
            msg_enable_ok: "有効化しました。再起動を推奨します。",
            msg_ti_fail: "権限昇格に失敗: ",
            msg_op_fail: "操作に失敗: ",
            msg_op_panic: "操作が異常終了しました",
            msg_watch_removed: "復元防止タスクを削除しました",
            msg_watch_installed: "復元防止タスクを有効化しました",
            error_title: "エラー",
            confirm_title: "操作の確認",
            confirm_disable: "Windows Defender を無効化しますか？\nシステムの安全性が低下します。現在の設定は自動でバックアップされ、いつでも復元できます。",
            log_backup: "現在の設定をバックアップ中...",
            log_restore_backup: "バックアップから元の設定を復元中...",
            startup_fail: "起動に失敗しました",
            btn_min: "—",
            btn_close: "✕",
            btn_copy: "コピー",
            btn_clear: "クリア",
            btn_more: "その他",
            restore_point_desc: "Defender Control - システム構成の変更",
            msg_tamper_title: "改ざん防止を先に無効化してください",
            msg_tamper_body: "Windows セキュリティの「改ざん防止」が有効です。本ツールの変更を自動的に元に戻すため、Defender を無効化できません。\n\nWindows セキュリティ → ウイルスと脅威の防止 → 設定の管理 を開き、「改ざん防止」をオフにしてから再試行してください。",
        },
        Lang::Ko => Strings {
            lang: Lang::Ko,
            app_title: "Defender Control",
            status_group: "상태",
            status_label: "상태",
            realtime_label: "실시간",
            antivirus_label: "백신",
            tamper_label: "변조",
            btn_disable: "Defender 비활성화",
            btn_enable: "Defender 활성화",
            btn_refresh: "새로고침",
            btn_watch_install: "복구 방지 작업 설치",
            btn_watch_remove: "복구 방지 작업 제거",
            service_group: "서비스 상태",
            log_group: "로그",
            col_name: "서비스 이름",
            col_status: "상태",
            col_startup: "시작 유형",
            status_disabled: "비활성화",
            status_protected: "보호 중",
            status_partial: "부분 활성화",
            status_on: "켜짐",
            status_off: "꺼짐",
            status_unknown: "--",
            log_ti_acquire: "TrustedInstaller 권한을 획득하는 중...",
            log_restore_point: "시스템 복원 지점을 생성하는 중...",
            log_disable_tamper: "변조 방지를 비활성화하는 중...",
            log_kill_smartscreen: "SmartScreen을 종료하는 중...",
            log_stop_services: "Defender 서비스를 중지하는 중...",
            log_strip_ppl: "서비스 PPL 보호를 제거하는 중...",
            log_disable_drivers: "Defender 커널 드라이버를 비활성화하는 중...",
            log_write_registry: "레지스트리 설정을 기록하는 중...",
            log_mp_pref: "MpPreference를 설정하는 중...",
            log_set_ifeo: "IFEO로 mpcmdrun.exe를 차단하는 중...",
            log_install_watch: "복구 방지 작업을 설치하는 중...",
            log_enable_tamper: "변조 방지를 복원하는 중...",
            log_restore_ppl: "서비스 PPL 보호를 복원하는 중...",
            log_start_services: "Defender 서비스를 복원하는 중...",
            log_enable_drivers: "Defender 드라이버를 복원하는 중...",
            log_remove_ifeo: "IFEO 차단을 제거하는 중...",
            log_remove_watch: "복구 방지 작업을 제거하는 중...",
            log_restore_altitude: "WdFilter Altitude를 복원하는 중...",
            msg_disable_ok: "비활성화되었습니다. 재부팅을 권장합니다.",
            msg_enable_ok: "활성화되었습니다. 재부팅을 권장합니다.",
            msg_ti_fail: "권한 상승 실패: ",
            msg_op_fail: "작업 실패: ",
            msg_op_panic: "작업이 비정상 종료되었습니다",
            msg_watch_removed: "복구 방지 작업이 제거되었습니다",
            msg_watch_installed: "복구 방지 작업이 설치되었습니다",
            error_title: "오류",
            confirm_title: "작업 확인",
            confirm_disable: "Windows Defender를 비활성화하시겠습니까?\n시스템 보안이 약해집니다. 현재 설정은 자동으로 백업되며 언제든지 복원할 수 있습니다.",
            log_backup: "현재 설정을 백업하는 중...",
            log_restore_backup: "백업에서 원래 설정을 복원하는 중...",
            startup_fail: "시작 실패",
            btn_min: "—",
            btn_close: "✕",
            btn_copy: "복사",
            btn_clear: "지우기",
            btn_more: "더보기",
            restore_point_desc: "Defender Control - 시스템 구성 변경",
            msg_tamper_title: "먼저 변조 방지를 끄세요",
            msg_tamper_body: "Windows 보안의 '변조 방지'가 켜져 있습니다. 이 도구가 적용한 변경을 자동으로 되돌리므로 Defender를 비활성화할 수 없습니다.\n\nWindows 보안 → 바이러스 및 위협 방지 → 설정 관리 에서 '변조 방지'를 끈 후 다시 시도하세요.",
        },
        _ => Strings {
            lang: Lang::En,
            app_title: "Defender Control",
            status_group: "Status",
            status_label: "Status",
            realtime_label: "Real-time",
            antivirus_label: "Antivirus",
            tamper_label: "Tamper",
            btn_disable: "Disable Defender",
            btn_enable: "Enable Defender",
            btn_refresh: "Refresh",
            btn_watch_install: "Install Watch Task",
            btn_watch_remove: "Remove Watch Task",
            service_group: "Services",
            log_group: "Log",
            col_name: "Service Name",
            col_status: "Status",
            col_startup: "Startup Type",
            status_disabled: "Disabled",
            status_protected: "Protected",
            status_partial: "Partial",
            status_on: "On",
            status_off: "Off",
            status_unknown: "--",
            log_ti_acquire: "Acquiring TrustedInstaller privilege...",
            log_restore_point: "Creating system restore point...",
            log_disable_tamper: "Disabling tamper protection...",
            log_kill_smartscreen: "Terminating SmartScreen...",
            log_stop_services: "Stopping Defender services...",
            log_strip_ppl: "Stripping service PPL protection...",
            log_disable_drivers: "Disabling Defender kernel drivers (Altitude)...",
            log_write_registry: "Writing registry configuration...",
            log_mp_pref: "Configuring MpPreference (PowerShell)...",
            log_set_ifeo: "Setting IFEO block for mpcmdrun.exe...",
            log_install_watch: "Installing watch task...",
            log_enable_tamper: "Restoring tamper protection...",
            log_restore_ppl: "Restoring service PPL protection...",
            log_start_services: "Starting Defender services...",
            log_enable_drivers: "Restoring Defender drivers...",
            log_remove_ifeo: "Removing IFEO block...",
            log_remove_watch: "Removing watch task...",
            log_restore_altitude: "Restoring WdFilter Altitude...",
            msg_disable_ok: "Defender disabled. Restart recommended.",
            msg_enable_ok: "Defender enabled. Restart recommended.",
            msg_ti_fail: "Privilege escalation failed: ",
            msg_op_fail: "Operation failed: ",
            msg_op_panic: "Operation panicked",
            msg_watch_removed: "Watch task removed",
            msg_watch_installed: "Watch task installed (auto-checks on startup)",
            error_title: "Error",
            confirm_title: "Confirm action",
            confirm_disable: "Disable Windows Defender?\nThis lowers your system's security. Your current configuration will be backed up automatically and can be restored anytime.",
            log_backup: "Backing up current configuration...",
            log_restore_backup: "Restoring original configuration from backup...",
            startup_fail: "Startup failed",
            btn_min: "—",
            btn_close: "✕",
            btn_copy: "Copy",
            btn_clear: "Clear",
            btn_more: "More",
            restore_point_desc: "Defender Control - System configuration change",
            msg_tamper_title: "Turn off Tamper Protection first",
            msg_tamper_body: "Windows Security's Tamper Protection is on. It automatically reverts the changes this tool makes, so Defender can't be disabled.\n\nOpen Windows Security \u{2192} Virus & threat protection \u{2192} Manage settings, turn off Tamper Protection, then try again.",
        },
    }
}