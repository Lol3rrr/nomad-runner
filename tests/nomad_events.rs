use nomad_runner::nomad;

#[test]
#[ignore = ""]
fn loading_events() {
    let client = nomad::Client::new("192.168.10.189".to_string(), 4646);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async move {
        let allocs = client
            .get_job_allocations("ci-4396812-d45a11b9803beb85")
            .await
            .unwrap();

        for alloc in allocs {
            dbg!(&alloc);

            let details = client.read_allocation(&alloc.id).await.unwrap();

            dbg!(&details);
        }

        todo!()
    });
}
