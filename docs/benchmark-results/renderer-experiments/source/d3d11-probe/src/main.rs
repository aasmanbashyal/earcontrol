#![windows_subsystem = "windows"]

use windows::{
    core::{w, BOOL},
    Win32::{
        Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::{
                D3D11CreateDeviceAndSwapChain, ID3D11DepthStencilView, ID3D11Device,
                ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC},
                IDXGIAdapter, IDXGISwapChain, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC,
                DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{GetDpiForSystem, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                PostQuitMessage, RegisterClassW, SetTimer, ShowWindow, SetWindowPos, IsWindowVisible, TranslateMessage,
                UnregisterClassW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, MSG,
                SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
                WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY, WM_TIMER,
                WNDCLASSW, WS_POPUP,
            },
        },
    },
};

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE | WM_TIMER => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn run() -> windows::core::Result<()> {
    let warp = std::env::args().any(|arg| arg == "--warp");
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?;
        let dpi = GetDpiForSystem().max(96);
        let width = 1160 * dpi / 96;
        let height = 780 * dpi / 96;
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class_name = w!("EarControlD3D11MemoryFloor");
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        // Match the borderless app's 1160x780 logical client area. No UI, text,
        // fonts, clipboard integration, custom accessibility adapter or media
        // capture is initialized by the probe (Windows may load UIA itself).
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("EarControl Direct3D11 memory control"),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width as i32,
            height as i32,
            None,
            None,
            Some(instance),
            None,
        )?;
        let description = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ..Default::default()
            },
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            OutputWindow: hwnd,
            Windowed: BOOL(1),
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            Flags: 0,
        };
        let mut swap_chain: Option<IDXGISwapChain> = None;
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature = D3D_FEATURE_LEVEL_11_0;
        D3D11CreateDeviceAndSwapChain(
            None::<&IDXGIAdapter>,
            if warp { D3D_DRIVER_TYPE_WARP } else { D3D_DRIVER_TYPE_HARDWARE },
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&description),
            Some(&mut swap_chain),
            Some(&mut device),
            Some(&mut feature),
            Some(&mut context),
        )?;
        let swap_chain = swap_chain.unwrap();
        let device = device.unwrap();
        let context = context.unwrap();
        let backbuffer: ID3D11Texture2D = swap_chain.GetBuffer(0)?;
        let mut target: Option<ID3D11RenderTargetView> = None;
        device.CreateRenderTargetView(&backbuffer, None, Some(&mut target))?;
        let target = target.unwrap();
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        // ProcessStartInfo.WindowStyle=Hidden applies to the first ShowWindow.
        // Show explicitly without activating the diagnostic window afterward.
        SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW)?;
        if !IsWindowVisible(hwnd).as_bool() {
            return Err(windows::core::Error::from_thread());
        }
        // Present enough frames to initialize both buffers and submit real GPU
        // work. This is an allocation floor, not an egui rendering benchmark.
        for _ in 0..3 {
            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None::<&ID3D11DepthStencilView>);
            context.ClearRenderTargetView(&target, &[0.96, 0.96, 0.96, 1.0]);
            swap_chain.Present(1, DXGI_PRESENT(0)).ok()?;
        }
        context.Flush();
        if let Some(path) = std::env::var_os("D3D11_PROBE_READY") {
            std::fs::write(
                path,
                format!(
                    "{{\"backend\":\"{}\",\"logical_width\":1160,\"logical_height\":780,\"width\":{width},\"height\":{height},\"dpi\":{dpi},\"feature_level\":{},\"presented_frames\":3,\"buffers\":2}}",
                    if warp { "d3d11_warp" } else { "d3d11_hardware" },
                    feature.0
                ),
            ).unwrap();
        }
        // A caller can close the owned process after sampling; this guard also
        // prevents leaving an unattended probe window behind.
        SetTimer(Some(hwnd), 1, 30_000, None);
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        context.ClearState();
        context.Flush();
        drop(target);
        drop(backbuffer);
        drop(swap_chain);
        drop(context);
        drop(device);
        let _ = UnregisterClassW(class_name, Some(instance));
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        if let Some(path) = std::env::var_os("D3D11_PROBE_READY") {
            let _ = std::fs::write(path, format!("D3D11 initialization failed: {error}"));
        }
        std::process::exit(1);
    }
}
