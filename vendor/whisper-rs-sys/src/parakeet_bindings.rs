// Fallback bindings for the Parakeet API used by Hushpen, from include/parakeet.h.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct parakeet_context {
    _unused: [u8; 0],
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct parakeet_state {
    _unused: [u8; 0],
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct parakeet_context_params {
    pub use_gpu: bool,
    pub gpu_device: ::std::os::raw::c_int,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct parakeet_token_data {
    pub id: i32,
    pub duration_idx: ::std::os::raw::c_int,
    pub duration_value: ::std::os::raw::c_int,
    pub frame_index: ::std::os::raw::c_int,
    pub p: f32,
    pub plog: f32,
    pub t0: i64,
    pub t1: i64,
    pub is_word_start: bool,
}
pub type parakeet_sampling_strategy = ::std::os::raw::c_uint;
pub const parakeet_sampling_strategy_PARAKEET_SAMPLING_GREEDY: parakeet_sampling_strategy = 0;
pub type parakeet_new_segment_callback = Option<
    unsafe extern "C" fn(
        *mut parakeet_context,
        *mut parakeet_state,
        ::std::os::raw::c_int,
        *mut ::std::os::raw::c_void,
    ),
>;
pub type parakeet_new_token_callback = Option<
    unsafe extern "C" fn(
        *mut parakeet_context,
        *mut parakeet_state,
        *const parakeet_token_data,
        *mut ::std::os::raw::c_void,
    ),
>;
pub type parakeet_progress_callback = parakeet_new_segment_callback;
pub type parakeet_encoder_begin_callback = Option<
    unsafe extern "C" fn(
        *mut parakeet_context,
        *mut parakeet_state,
        *mut ::std::os::raw::c_void,
    ) -> bool,
>;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct parakeet_full_params {
    pub strategy: parakeet_sampling_strategy,
    pub n_threads: ::std::os::raw::c_int,
    pub offset_ms: ::std::os::raw::c_int,
    pub duration_ms: ::std::os::raw::c_int,
    pub no_context: bool,
    pub audio_ctx: ::std::os::raw::c_int,
    pub new_segment_callback: parakeet_new_segment_callback,
    pub new_segment_callback_user_data: *mut ::std::os::raw::c_void,
    pub new_token_callback: parakeet_new_token_callback,
    pub new_token_callback_user_data: *mut ::std::os::raw::c_void,
    pub progress_callback: parakeet_progress_callback,
    pub progress_callback_user_data: *mut ::std::os::raw::c_void,
    pub encoder_begin_callback: parakeet_encoder_begin_callback,
    pub encoder_begin_callback_user_data: *mut ::std::os::raw::c_void,
    pub abort_callback: ggml_abort_callback,
    pub abort_callback_user_data: *mut ::std::os::raw::c_void,
}
unsafe extern "C" {
    pub fn parakeet_context_default_params() -> parakeet_context_params;
    pub fn parakeet_full_default_params(
        strategy: parakeet_sampling_strategy,
    ) -> parakeet_full_params;
    pub fn parakeet_init_from_file_with_params(
        path: *const ::std::os::raw::c_char,
        params: parakeet_context_params,
    ) -> *mut parakeet_context;
    pub fn parakeet_free(ctx: *mut parakeet_context);
    pub fn parakeet_full(
        ctx: *mut parakeet_context,
        params: parakeet_full_params,
        samples: *const f32,
        n_samples: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    pub fn parakeet_full_n_segments(ctx: *mut parakeet_context) -> ::std::os::raw::c_int;
    pub fn parakeet_full_get_segment_t0(
        ctx: *mut parakeet_context,
        segment: ::std::os::raw::c_int,
    ) -> i64;
    pub fn parakeet_full_get_segment_t1(
        ctx: *mut parakeet_context,
        segment: ::std::os::raw::c_int,
    ) -> i64;
    pub fn parakeet_full_get_segment_text(
        ctx: *mut parakeet_context,
        segment: ::std::os::raw::c_int,
    ) -> *const ::std::os::raw::c_char;
    pub fn parakeet_full_n_tokens(
        ctx: *mut parakeet_context,
        segment: ::std::os::raw::c_int,
    ) -> ::std::os::raw::c_int;
    pub fn parakeet_full_get_token_data(
        ctx: *mut parakeet_context,
        segment: ::std::os::raw::c_int,
        token: ::std::os::raw::c_int,
    ) -> parakeet_token_data;
    pub fn parakeet_full_get_token_text(
        ctx: *mut parakeet_context,
        segment: ::std::os::raw::c_int,
        token: ::std::os::raw::c_int,
    ) -> *const ::std::os::raw::c_char;
    pub fn parakeet_log_set(callback: ggml_log_callback, user_data: *mut ::std::os::raw::c_void);
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(::std::mem::size_of::<parakeet_context_params>() == 8);
    assert!(::std::mem::size_of::<parakeet_token_data>() == 48);
    assert!(::std::mem::size_of::<parakeet_full_params>() == 104);
    assert!(::std::mem::offset_of!(parakeet_full_params, abort_callback_user_data) == 96);
};
