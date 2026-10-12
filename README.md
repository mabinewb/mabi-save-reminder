# 스더저장해

Windows 10/11 x64용 Rust 프로그램입니다. 별도 런타임 설치 없이 `스더저장해.exe`를 실행하세요. 기본 실행은 트레이에만 표시되며, 이미 마비노기가 실행 중이면 바로 알림이 뜹니다.

[최신 버전 다운로드](https://github.com/mabinewb/mabi-save-reminder/releases/latest)에서 `스더저장해.zip`을 받아 압축을 푸세요. 실행 파일과 `settings.ini`를 같은 폴더에 두고 실행하면 됩니다.

## 동작

- 메인 모니터 상단에 좌우 전체 폭으로 **겜켰으면 스더 설정 저장하세요**를 표시합니다.
- 문구는 화면 너비에 비례하는 매우 큰 굵은 글자입니다. 1920px 화면에서 약 106px, 3840px 화면에서 약 213px이며 배너 높이도 함께 커집니다.
- 빨강/노랑 배경을 650ms마다 교체합니다. 게임 입력의 포커스를 빼앗지 않습니다.
- 알림 클릭, 5분 경과, 감지한 게임 프로세스가 모두 종료되는 경우에 닫힙니다.
- 게임 종료 확인 간격은 2초입니다. 닫은 뒤 같은 게임 프로세스에는 반복해서 알리지 않습니다. 새 게임 프로세스가 실행되면 다시 알립니다.
- 웹 브라우저나 게임에 코드를 주입하지 않고 Windows 프로세스 목록과 창 정보만 조회합니다.

## 트레이 메뉴

트레이 아이콘을 우클릭하세요. 아이콘이 안 보이면 작업 표시줄의 숨겨진 아이콘 영역을 확인하세요.

- **알림 테스트**: 게임 없이도 알림을 미리 봅니다. 테스트 알림은 게임 종료와 무관하게 클릭 또는 5분 뒤 닫힙니다.
- **알림 닫기**: 현재 알림을 닫습니다.
- **설정 파일 열기 / 설정 다시 읽기**: `settings.ini`를 수정하고 다시 읽기를 누르면 적용됩니다.
- **Windows 로그인 시 자동 실행**: 체크하면 현재 사용자 로그인 시 트레이로 시작합니다. 해제하면 등록을 제거합니다. 기본은 해제입니다. 관리자 권한은 필요하지 않습니다.
- **종료**: 프로그램을 완전히 종료합니다. 자동 실행 등록은 유지됩니다.

자동 실행을 켠 뒤 프로그램 폴더를 옮겼다면, 새 위치에서 실행해 옵션을 한 번 끄고 다시 켜세요. 삭제할 때도 먼저 자동 실행 체크를 해제하세요. 설정 파일은 실행 파일 옆에 두세요.

## 게임 감지

2초마다 `Client.exe`를 찾고 실행 파일 경로에 `mabinogi`가 포함되는지 확인합니다. 이름과 경로는 `settings.ini`에서 바꿀 수 있습니다. 예를 들어 설치 경로가 `D:\게임\마비노기\Client.exe`라면 `path_contains=마비노기`로 설정합니다.

실행 경로를 조회할 수 없는 경우에는 같은 프로세스 ID의 표시된 창 제목에 `마비노기` 또는 `mabinogi`가 들어 있는지를 보조 판정으로 사용합니다. 이름이 같은 다른 프로그램은 경로 또는 게임 창 제목까지 일치해야 감지됩니다. 이름만으로 감지하려면 `path_contains=`를 비우세요.

## 화면 모드 지원 범위

창모드와 테두리 없는 전체화면에서는 최상위 Windows 창으로 표시합니다. Windows 전체화면 최적화가 적용된 전체화면도 표시될 수 있습니다. **실제 독점 전체화면에는 표시를 보장하지 않습니다.** 알림이 가려지면 게임을 테두리 없는 전체화면/창모드로 바꾸세요. 다른 프로그램의 최상위 창이나 시스템 보안 화면에도 표시를 보장하지 않습니다.

참고: [Microsoft 전체화면 최적화 설명](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations/), [SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos).

## 설정

`blink_ms=0`으로 설정하면 깜빡임을 끕니다. 0 이외의 값은 500~10000ms로 제한됩니다. `timeout_seconds=300`은 5분이고, 1~3600초 범위로 설정할 수 있습니다. 잘못된 숫자는 기본값을 사용합니다. 자동 실행은 Windows 사용자 레지스트리 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`의 `MabiSaveReminder` 항목으로 관리됩니다.

## 소스 빌드

Rust stable과 Visual Studio C++ Build Tools가 설치된 일반 MSVC 환경에서:

```powershell
cargo test --locked
cargo build --release --locked
Copy-Item target\release\mabi-save-reminder.exe 스더저장해.exe
```

GNU 환경에서는 Rust GNU 도구 체인과 MinGW 링커/dlltool이 필요합니다. 제공된 실행 파일은 Rust GNU와 LLVM MinGW로 빌드했습니다. Cargo.lock에 의존성 버전을 고정했습니다.

명령줄 `스더저장해.exe --test`는 검증용 미리보기입니다. 이 모드에만 작업 표시줄 항목이 생깁니다. 일반 사용은 인자 없이 실행하세요. 트레이 메뉴의 알림 테스트는 일반 트레이 모드를 유지합니다.

실행 파일과 `settings.ini`만 있으면 실행할 수 있습니다. 아이콘은 실행 파일에 포함되어 있습니다. `src`, `assets`, `build.rs`, `resources.rc`, `Cargo.toml`, `Cargo.lock`은 수정/빌드용입니다.

아이콘 원본: https://mabires2.pril.cc/optionweaponshapeimage/kr/2804/2804.png (사용자 지정 원본을 16~256px ICO로 리사이즈).
