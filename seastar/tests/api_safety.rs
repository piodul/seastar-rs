use seastar::{engine_is_ready, AppTemplate};

#[test]
fn test_engine_is_ready_only_for_thread_in_runtime() {
    assert!(!engine_is_ready());

    std::thread::spawn(|| {
        assert!(!engine_is_ready());
        let mut app = AppTemplate::default();
        let fut = async {
            assert!(engine_is_ready());
            Ok(())
        };
        let args = vec!["test"];
        app.run_void(&args, fut);
        // Currently, Seastar does not perform a cleanup of its thread locals,
        // which unfortunately means the following assertion will pass.
        // Should it fail, we must:
        // 1) negate its condition
        // 2) review other code in the project that also follows
        //    this assumption and modify it accordingly.
        assert!(engine_is_ready());
    })
    .join()
    .unwrap();

    assert!(!engine_is_ready());
}
