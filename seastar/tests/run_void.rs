use seastar::AppTemplate;

#[test]
fn test_run_void() {
    std::thread::spawn(|| {
        let mut app = AppTemplate::default();
        let args = vec!["test"];
        let fut = async { Ok(()) };
        assert_eq!(app.run_void(&args[..], fut), 0);
    })
    .join()
    .unwrap();
}
