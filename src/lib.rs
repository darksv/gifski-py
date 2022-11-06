use std::thread::JoinHandle;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes};
use pyo3::wrap_pyfunction;
use pyo3::exceptions::PyRuntimeError;

#[pyclass]
struct Handle {
    handle: Option<JoinHandle<Result<Box<[u8]>, ()>>>,
    is_ready: std::sync::mpsc::Receiver<()>,
    collector: Option<gifski::Collector>,
    width: u32,
    height: u32,
    pts: f64,
    frames: usize,
}

#[pyfunction]
fn new_encoder(width: u32, height: u32) -> Option<Handle> {
    let settings = gifski::Settings {
        width: Some(width),
        height: Some(height),
        quality: 100,
        repeat: gifski::Repeat::Infinite,
        fast: false,
    };

    let (collector, writer) = gifski::new(settings).ok()?;
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || -> Result<_, ()> {
        let mut reporter = gifski::progress::NoProgress {};
        let mut encoded = Vec::new();
        writer.write(&mut encoded, &mut reporter).map_err(|_| ())?;
        tx.send(()).unwrap();
        Ok(encoded.into_boxed_slice())
    });

    Some(Handle {
        handle: Some(handle),
        is_ready: rx,
        collector: Some(collector),
        width,
        height,
        pts: 0.0,
        frames: 0,
    })
}

#[pyfunction]
fn add_frame(handle: &mut Handle, data: PyObject, duration: f64) {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let data = data.cast_as::<PyBytes>(py).unwrap().as_bytes();
    assert_eq!(data.len(), handle.width as usize * handle.height as usize * 4);
    assert!(handle.collector.is_some());

    let image = data
        .chunks_exact(4)
        .map(|pix| rgb::RGBA8::new(pix[0], pix[1], pix[2], pix[3]))
        .collect();

    let img = imgref::ImgVec::new(
        image,
        handle.width as usize,
        handle.height as usize,
    );
    handle.collector.as_mut().unwrap().add_frame_rgba(
        handle.frames,
        img,
        handle.pts,
    ).unwrap();
    handle.pts += duration;
    handle.frames += 1;
}

#[pyfunction]
fn finish(handle: &mut Handle) {
    handle.collector.take().unwrap();
}

#[pyfunction]
fn get_result(handle: &mut Handle) -> PyResult<PyObject> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    if handle.is_ready.try_recv().is_ok() {
        let handle = handle.handle.take().unwrap();
        let result = handle.join().unwrap();
        match result {
            Ok(result) => Ok(PyBytes::new(py, &result).into_py()),
            Err(e) => Err(PyRuntimeError::new_err("some error"))
        }
    } else {
        Ok(PyBool::new(py, false).into())
    }
}

#[pymodule]
fn gifski(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(new_encoder, m)?).unwrap();
    m.add_function(wrap_pyfunction!(add_frame, m)?).unwrap();
    m.add_function(wrap_pyfunction!(finish, m)?).unwrap();
    m.add_function(wrap_pyfunction!(get_result, m)?).unwrap();
    Ok(())
}