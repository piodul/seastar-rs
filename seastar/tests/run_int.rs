use seastar::AppTemplate;

#[test]
fn test_run_int() {
    std::thread::spawn(|| {
        let mut app = AppTemplate::default();
        let args = vec!["test"];
        let fut = async { Ok(42) };
        assert_eq!(app.run_int(&args[..], fut), 42);
    })
    .join()
    .unwrap();
}
