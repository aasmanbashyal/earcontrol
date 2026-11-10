# Full QA record

Local QA completed on `feat/rust-native-ui`, November 10, 2026, with the limits recorded below. This pass began from build `87674e6`. Formatting, all-target Clippy, the complete ordinary Rust suite, PE fixtures, the separate native Windows smoke test, repeated physical earbud control, targeted live UI checks and package verification pass. This is not a completed clean-machine installation or upgrade qualification.

The earlier [input-recovery report](input-recovery.md) describes the preceding tested artifact. Its hashes and results do not identify the package being rebuilt during this QA pass. [Optimization measurements](optimization.md) remain tied to their recorded build; performance benchmarks have not been repeated for these fixes.

## Results

| Area | Evidence | Current status |
| --- | --- | --- |
| Starting build | 72 ordinary Rust tests passed; the interactive Windows test passed separately | Repeated at the start of this pass |
| Final automated checks | Formatting and all-target Clippy passed; 82 Rust tests passed, with one interactive test ignored in the ordinary run | Passed |
| Native capture and runtime | New Stop/retry cooldown and held-key regressions passed; the separate static-CRT debug Windows smoke test passed | Passed |
| Settings persistence | New size-limit, invalid UTF-8 and locked-file tests passed in the complete suite | Passed |
| Practice keyboard focus | New full-frame arrow-navigation regression passed; live Right, Right, Left reached slides 2, 3, 2 with focus retained | Passed |
| Physical headphone Monitor | Nothing earbuds produced Next and Previous reports in Monitor | Passed for the tested device and gestures |
| Physical control of Practice | After starting Control, five new shortcut inputs raised the deck count from 4 to 9, including Left; the tester confirmed repeated movement in both directions | Passed for the tested device and setup |
| Existing Practice target | The Practice button refocused the selected deck without stopping Control | Passed live |
| UI editing | Repeat toggle, slider, light/dark themes, Ctrl+K recording and draft discard worked in the release window | Passed live |
| Practice text and keyboard access | Tab left the slide, Unicode text entry worked, and Left in the editor did not move the slide | Passed live |
| Portable dependency checks | Seven PE fixtures passed; release imports contain only Windows DLLs; ZIP contents and all three artifact checksums passed | Passed |

The Windows smoke test delivered Right, Left and Unicode to its own window, exercised real SMTC commands through Monitor, checked Stop/restart and retry state, and recovered a deliberately removed keyboard hook. It addressed a known EarControl media session directly. This test passed again separately in the final static-CRT debug build. Physical headphone routing and release UI behavior were checked separately.

The final local artifacts are:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `EarControl-3.0.0/EarControl.exe` | 2,373,632 | `57cee6784656616d9e0e20e05de227ab3f18e2668fd4efbac9e2b55e48a74d93` |
| `EarControl-3.0.0-win-x64.zip` | 1,257,301 | `6e91a3291ef7c2b890fb237c6b142ca084cdc93611d1fe6f1bbe2e05235f76c6` |
| `EarControl_3.0.0_x64-setup.exe` | 1,046,676 | `b04344cfe43d9e163cb8d9756de281ff9a948fa81d27348d4e40788cf867f123` |

The delivery copy `EarControl-3.0.0-qa-win-x64.zip` is byte-identical to the versioned ZIP. The live candidate and packaged executable have the same hash. The older `input-ui-fix` delivery copy remains a historical artifact. No branch, tag or release was published.

## Findings and changes

### A delayed report could restore an old cooldown

A callback could read the active session, pause, and then update the duplicate filter after Stop had reset it. The stale timestamp could suppress the first command after a retry. The acceptance path now holds the runtime session lock while checking its generation and updating the duplicate filter. It performs no native call, wait or repaint while holding that lock.

The deterministic regression resumes an old-generation report after Stop and activation of the next session. It checks that the stale report is rejected and the first new report passes. It needs no desktop window or input injection.

### Stop could lose a held key's release decision

A media-key down consumed during control could be followed by Stop and an auto-repeat down before the release. The old code overwrote the remembered consumption decision while listening was disabled.

Stopped capture now keeps consuming the remaining repeats and release of that already-consumed key, without generating another command. A new listening generation still accepts a fresh down to recover from a missing device release. Keys whose down passed through remain unconsumed. Injected-key filtering is unchanged. The regression covers each of these cases.

### Practice moved keyboard focus after an arrow

During the physical control check, Activity reported successful input delivery, but the Practice deck advanced only once. After the first Right, focus moved into its text editor; later arrows were handled there instead of navigating slides. An executed Activity entry confirms input submission, not the intended visual result in the target.

The new regression sends consecutive Right, Right and Left events through complete egui frames. It initially failed, unlike the older handler-level test, and passes after the fix. The change keeps deck navigation keys with the slide while retaining Tab navigation and normal arrow editing when the text field is deliberately focused. The regression passed in the final complete suite. A live check of the rebuilt candidate then reached slides 2, 3 and 2, with received counts 1, 2 and 3 and slide focus retained.

The first physical attempt against the rebuilt candidate was still in Monitor, with no Control-start log, so it did not test target control. After starting Control with the Practice target selected, five new shortcut inputs increased the deck's count from 4 to 9, including Left. The tester confirmed: "Yes, both directions work repeatedly." A later Activity snapshot contained nine executed Next/Previous events from that session. When focus moved to the main window, subsequent physical commands were logged as skipped because the chosen window was inactive. This successful retest is separate from the earlier focus failure and Monitor-only attempt.

The Practice button also stopped control every time it was pressed, including when the selected Practice window already existed. The change makes that case refocus the existing target. Live verification confirmed that Control stays active. Opening a new deck or changing the target still stops the previous session.

### UI review controls, focus and refresh

The import confirmation now limits its width and scrolls the profile-name list, keeping Cancel and Replace and import visible for a valid 32-profile import with long names at the minimum window size. A regression checks those controls' accessible bounds and confirms that reviewing an import leaves the draft unchanged.

Custom mapping, macro and Devices controls now show keyboard-focus outlines; the Devices shortcut also exposes its button name to accessibility tools. Tests cover the accessible names, focus outlines and keyboard activation of mapping and macro controls. Switching pages resets the destination page to the top while ordinary redraws retain the active page's scroll position. A separate regression covers that behavior.

Background device scans explicitly request a repaint of the main viewport, so completion refreshes Devices even when another viewport is active. Activity's count is now labelled "Recent events" because its list includes status entries. These changes passed the complete automated checks.

Live release checks verified padded action/shortcut fields in both themes, toggling repeat delay, changing its slider value, recording Ctrl+K and discarding temporary draft changes. In Practice, Tab visibly focused Previous; deliberately focusing the editor allowed `QA नमस्ते 🦀` to be entered, and Left kept the slide and received-shortcut count unchanged. Closing Practice cleared its target in the main window. UI tests used an isolated settings copy, preserving personal profiles.

The Ctrl+Shift+F5 recording attempt was intercepted by a separate desktop utility and changed its performance mode. Its prior mode and Windows power plan were restored and verified from saved state and logs. The successful recording check used Ctrl+K. No performance measurements were taken during the changed mode.

### Portable packages depended on an unbundled runtime

The preceding executable imported `VCRUNTIME140.dll`, but the portable ZIP and installer did not contain that dependency. Inspection established the dependency; a clean-machine launch failure was not separately reproduced.

The Windows x64 Cargo configuration now requests static CRT linking. Packaging checks inspect ordinary and delay-loaded imports and reject Visual C++ runtime DLL dependencies. They also check the x64 GUI executable format. The old package is correctly rejected. The rebuilt release passed and imported only Windows DLLs. Packaging and release verification passed for the ZIP and installer. Installer checksum verification does not execute an installation or verify its extracted payload after installation.

Seven synthetic PE fixtures passed:

1. Accept an x64 GUI file with ordinary and delay-loaded Windows system DLL imports.
2. Reject an ordinary `VCRUNTIME140.dll` import.
3. Reject a delay-loaded `vcruntime140_1.dll` import.
4. Reject an x86 machine type.
5. Reject a file without a valid Windows executable header.
6. Reject truncated PE headers.
7. Reject an import-name address outside a file-backed section.

### Persistence failure coverage

The added tests use temporary files only. They verify that a valid JSON document padded to exactly 2 MiB can be read and one extra byte is rejected; a valid settings object whose serialized form exceeds the limit cannot replace the previous file; invalid UTF-8 leaves the original bytes untouched and produces a defaults warning; and a Windows sharing-denied replacement preserves the old file and cleans up its temporary file.

Existing tests also cover missing files, malformed or unsupported settings, schema validation, Unicode round trips, atomic replacement, and disabled repeat delay surviving save/reload. These checks do not simulate power loss or storage-device failure.

## Activity report

The reported unexpected Activity clearing has not been reproduced. Source inspection found one production call to `clear_events`, from the explicit Clear Activity button. Stop, retry and screen navigation have no automatic clear call.

Activity is intentionally held in memory, limited to the newest 250 entries, and lost when the process exits. Button check filters outcomes by capture session; that filtering does not erase Activity. The unexplained report remains unresolved rather than being marked fixed.

## Limits and follow-up checks

All five screens and the import dialog passed automated minimum-size and theme layout checks. The live edge-drag attempt did not resize the window through the automation tool, so this pass does not establish manual edge resizing or every DPI configuration. Import/export validation, profile CRUD, malformed drafts, cancellation and focus-loss behavior have automated coverage; a complete native file-dialog round trip and external-editor clipboard test were not repeated live.

Actual installer execution, a 2.x upgrade and uninstall have not been tested. Changing only the install directory is not an isolated installer test because registry and Start-menu paths still affect the current account. Use a disposable VM or Windows Sandbox for that work. Hardware disconnect/reconnect, sleep/resume recovery, other headphone models and drivers, and hosted CI also remain untested in this pass.

Reproduction commands are in [the contributor guide](../CONTRIBUTING.md), [release verification](releasing.md) and [input diagnosis](input-recovery.md). Synthetic checks and a successful Monitor observation do not replace an end-to-end target behavior test.
