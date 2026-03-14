use std::{cell::RefCell, collections::HashMap, mem, rc::Rc};

use async_channel::Sender;
use futures::Stream;
use libpulse_binding::{
    callbacks::ListResult,
    context::{
        subscribe::{Facility, InterestMaskSet},
        Context, FlagSet, State,
    },
    proplist::properties,
};
use libpulse_glib_binding::Mainloop;
use waybar_cffi::gtk::glib;

#[derive(Debug, Clone)]
pub struct SinkInput {
    pub index: u32,
    pub muted: bool,
}

pub type AudioState = HashMap<u32, Vec<SinkInput>>;

thread_local! {
    static PA_MAINLOOP: RefCell<Option<Mainloop>> = RefCell::new(None);
    static PA_CONTEXT: RefCell<Option<Context>> = RefCell::new(None);
}

pub fn create_stream() -> impl Stream<Item = AudioState> {
    let (tx, rx) = async_channel::unbounded();
    glib::idle_add_local_once(move || setup_pulse_audio(tx));
    async_stream::stream! {
        while let Ok(state) = rx.recv().await {
            yield state;
        }
    }
}

pub fn toggle_mute(sink_inputs: &[SinkInput]) {
    let all_muted = sink_inputs.iter().all(|s| s.muted);
    let target_mute = !all_muted;
    PA_CONTEXT.with(|ctx| {
        if let Some(ctx) = ctx.borrow().as_ref() {
            let mut introspector = ctx.introspect();
            for s in sink_inputs {
                let _ = introspector.set_sink_input_mute(s.index, target_mute, None);
            }
        }
    });
}

fn setup_pulse_audio(tx: Sender<AudioState>) {
    let mainloop = match Mainloop::new(None) {
        Some(m) => m,
        None => {
            tracing::error!("failed to create PulseAudio GLib mainloop");
            return;
        }
    };

    let mut context = match Context::new(&mainloop, "niri-waybar-windowlist") {
        Some(c) => c,
        None => {
            tracing::error!("failed to create PulseAudio context");
            return;
        }
    };

    let tx_state = tx.clone();
    context.set_state_callback(Some(Box::new(move || {
        let state = PA_CONTEXT.with(|ctx| ctx.borrow().as_ref().map(|c| c.get_state()));
        match state {
            Some(State::Ready) => {
                on_context_ready(tx_state.clone());
            }
            Some(State::Failed) | Some(State::Terminated) => {
                tracing::error!("PulseAudio context disconnected");
                let _ = tx_state.try_send(HashMap::new());
            }
            _ => {}
        }
    })));

    if let Err(e) = context.connect(None, FlagSet::NOFLAGS, None) {
        tracing::error!("failed to connect to PulseAudio: {:?}", e);
        return;
    }

    PA_MAINLOOP.with(|ml| *ml.borrow_mut() = Some(mainloop));
    PA_CONTEXT.with(|ctx| *ctx.borrow_mut() = Some(context));
}

fn on_context_ready(tx: Sender<AudioState>) {
    query_sink_inputs(tx.clone());

    PA_CONTEXT.with(|ctx| {
        let mut ctx_ref = ctx.borrow_mut();
        if let Some(ctx) = ctx_ref.as_mut() {
            let _ = ctx.subscribe(InterestMaskSet::SINK_INPUT, |_| {});
            let tx_cb = tx;
            ctx.set_subscribe_callback(Some(Box::new(move |facility, _op, _index| {
                if matches!(facility, Some(Facility::SinkInput)) {
                    query_sink_inputs(tx_cb.clone());
                }
            })));
        }
    });
}

fn read_parent_pid(pid: u32) -> Option<u32> {
    let content = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    content.lines().find_map(|line| {
        line.strip_prefix("PPid:")
            .and_then(|rest| rest.trim().parse::<u32>().ok())
            .filter(|&ppid| ppid > 1)
    })
}

fn query_sink_inputs(tx: Sender<AudioState>) {
    PA_CONTEXT.with(|ctx| {
        let ctx_ref = ctx.borrow();
        if let Some(ctx) = ctx_ref.as_ref() {
            let introspector = ctx.introspect();
            let accumulator: Rc<RefCell<Vec<(u32, SinkInput)>>> = Rc::new(RefCell::new(Vec::new()));
            let _ = introspector.get_sink_input_info_list(move |result| match result {
                ListResult::Item(info) => {
                    if info.corked {
                        return;
                    }
                    if let Some(pid_str) = info.proplist.get_str(properties::APPLICATION_PROCESS_ID)
                    {
                        if let Ok(pid) = pid_str.trim().parse::<u32>() {
                            accumulator.borrow_mut().push((pid, SinkInput { index: info.index, muted: info.mute }));
                        }
                    }
                }
                ListResult::End => {
                    let items = mem::take(&mut *accumulator.borrow_mut());
                    let mut state: AudioState = HashMap::new();
                    for (pid, sink_input) in items {
                        let mut current = pid;
                        loop {
                            state.entry(current).or_default().push(sink_input.clone());
                            match read_parent_pid(current) {
                                Some(parent) => current = parent,
                                None => break,
                            }
                        }
                    }
                    let _ = tx.try_send(state);
                }
                ListResult::Error => {
                    tracing::error!("error querying PulseAudio sink inputs");
                }
            });
        }
    });
}
