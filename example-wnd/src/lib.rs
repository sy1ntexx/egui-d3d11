#![allow(unused)]

use std::intrinsics::transmute;

use egui::{Color32, CtxRef, Pos2, Rect, Stroke};
use egui_d3d11::DirectX11App;
use radon::{internal::alloc_console, pattern::Pattern};
use windows::{
    core::HRESULT,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::{
            Direct3D11::ID3D11Device,
            Dxgi::{Common::DXGI_FORMAT, IDXGISwapChain},
        },
        UI::WindowsAndMessaging::{CallWindowProcW, SetWindowLongPtrA, GWLP_WNDPROC, WNDPROC},
    },
};

#[no_mangle]
unsafe extern "stdcall" fn DllMain(hinst: usize, reason: u32) -> i32 {
    if reason == 1 {
        std::thread::spawn(move || unsafe { main_thread(hinst) });
    }

    1
}

static mut APP: Option<DirectX11App> = None;
static mut OLD_WND_PROC: Option<WNDPROC> = None;

type FnPresent = unsafe extern "stdcall" fn(IDXGISwapChain, u32, u32) -> HRESULT;
static mut O_PRESENT: Option<FnPresent> = None;

type FnResizeBuffers =
    unsafe extern "stdcall" fn(IDXGISwapChain, u32, u32, u32, DXGI_FORMAT, u32) -> HRESULT;
static mut O_RESIZE_BUFFERS: Option<FnResizeBuffers> = None;

unsafe extern "stdcall" fn hk_present(
    swap_chain: IDXGISwapChain,
    sync_interval: u32,
    flags: u32,
) -> HRESULT {
    let device: ID3D11Device = swap_chain.GetDevice().unwrap();
    let mut context = None;
    device.GetImmediateContext(&mut context);

    if OLD_WND_PROC.is_none() {
        APP = Some(DirectX11App::new(ui, &swap_chain, &device));

        let hwnd = swap_chain.GetDesc().unwrap().OutputWindow;
        if hwnd.is_invalid() {
            panic!("Invalid window handle.");
        }

        OLD_WND_PROC = Some(transmute(SetWindowLongPtrA(
            hwnd,
            GWLP_WNDPROC,
            hk_wnd_proc as usize as _,
        )));
    }

    APP.as_ref()
        .unwrap()
        .present(&swap_chain, sync_interval, flags);

    O_PRESENT.as_ref().unwrap()(swap_chain, sync_interval, flags)
}

unsafe extern "stdcall" fn hk_resize_buffers(
    swap_chain: IDXGISwapChain,
    buffer_count: u32,
    width: u32,
    height: u32,
    new_format: DXGI_FORMAT,
    swap_chain_flags: u32,
) -> HRESULT {
    APP.as_ref().unwrap().resize_buffers(
        &swap_chain,
        buffer_count,
        width,
        height,
        new_format,
        swap_chain_flags,
        O_RESIZE_BUFFERS.unwrap(),
    )
}

unsafe extern "stdcall" fn hk_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if APP.as_ref().unwrap().wnd_proc(hwnd, msg, wparam, lparam) {
        CallWindowProcW(OLD_WND_PROC.unwrap(), hwnd, msg, wparam, lparam)
    } else {
        LRESULT(0)
    }
}

fn ui(ctx: &CtxRef) {
    ctx.debug_painter().rect(
        Rect {
            min: Pos2::new(10.0, 10.0),
            max: Pos2::new(150.0, 150.0),
        },
        10.0,
        Color32::from_rgba_premultiplied(255, 0, 0, 255),
        Stroke::none(),
    );
}

unsafe fn main_thread(_hinst: usize) {
    alloc_console();

    eprintln!("Hello World!");

    let present = radon::internal::find_pattern(
        "gameoverlayrenderer64.dll",
        Pattern::from_ida_style("48 89 6C 24 18 48 89 74 24 20 41 56 48 83 EC 20 41"),
    )
    .unwrap_or_else(|_| {
        radon::internal::find_pattern(
            "dxgi.dll",
            Pattern::from_ida_style("48 89 5C 24 10 48 89 74 24 20 55 57 41 56"),
        )
        .unwrap()
    })
    .unwrap() as usize;

    eprintln!("Present: {:X}", present);

    let swap_buffers = radon::internal::find_pattern(
        "gameoverlayrenderer64.dll",
        Pattern::from_ida_style(
            "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 41 56 41 57 48 83 EC 30 44",
        ),
    )
    .unwrap_or_else(|_| {
        radon::internal::find_pattern(
            "dxgi.dll",
            Pattern::from_ida_style("48 8B C4 55 41 54 41 55 41 56 41 57 48 8D 68 B1 48 81 EC C0"),
        )
        .unwrap()
    })
    .unwrap() as usize;

    eprintln!("Buffers: {:X}", swap_buffers);

    sunshine::create_hook(
        sunshine::HookType::Compact,
        transmute::<_, FnPresent>(present),
        hk_present as FnPresent,
        &mut O_PRESENT,
    )
    .unwrap();

    sunshine::create_hook(
        sunshine::HookType::Compact,
        transmute::<_, FnResizeBuffers>(swap_buffers),
        hk_resize_buffers as FnResizeBuffers,
        &mut O_RESIZE_BUFFERS,
    )
    .unwrap();

    #[allow(clippy::empty_loop)]
    loop {}
}