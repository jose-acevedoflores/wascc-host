extern crate dummy_fs_provider;
use reqwest;
use std::collections::HashMap;
use std::error::Error;
use wascc_codec::core::CapabilityConfiguration;
use wascc_host::Host;

pub(crate) fn stock_host() -> Result<(), Box<dyn Error>> {
    let host = crate::common::gen_stock_host(9090)?;
    assert_eq!(2, host.actors().len());
    if let Some(ref claims) =
        host.claims_for_actor("MB4OLDIC3TCZ4Q4TGGOVAZC43VXFE2JQVRAXQMQFXUCREOOFEKOKZTY2")
    {
        let md = claims.metadata.as_ref().unwrap();
        assert!(md
            .caps
            .as_ref()
            .unwrap()
            .contains(&"wascc:http_server".to_string()));
    }

    std::thread::sleep(::std::time::Duration::from_millis(500));

    let resp = reqwest::blocking::get("http://localhost:9090")?;
    assert!(resp.status().is_success());
    assert_eq!(resp.text()?,
        "{\"method\":\"GET\",\"path\":\"/\",\"query_string\":\"\",\"headers\":{\"accept\":\"*/*\",\"host\":\"localhost:9090\"},\"body\":[]}"
    );
    host.shutdown()?;
    std::thread::sleep(::std::time::Duration::from_millis(500));
    Ok(())
}

pub(crate) fn kv_host() -> Result<(), Box<dyn Error>> {
    use redis::Commands;

    let host = crate::common::gen_kvcounter_host(8083, Host::new())?;
    std::thread::sleep(::std::time::Duration::from_millis(100));
    let key = uuid::Uuid::new_v4().to_string();
    let rkey = format!(":{}", key); // the kv wasm logic does a replace on '/' with ':'
    let url = format!("http://localhost:8083/{}", key);
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_connection()?;

    let mut resp = reqwest::blocking::get(&url)?;
    assert!(resp.status().is_success());
    reqwest::blocking::get(&url)?;
    resp = reqwest::blocking::get(&url)?; // counter should be at 3 now
    assert!(resp.status().is_success());
    assert_eq!(resp.text()?, "{\"counter\":3}");
    host.shutdown()?;

    let _: () = con.del(&rkey)?;
    Ok(())
}

pub(crate) fn propagate_error_from_provider() -> Result<(), Box<dyn Error>> {
    // In this test the host manually triggers the dummy_actor to call a fake blobstore provider
    // that always returns an error. The test verifies that the error is correctly propagated
    // back to the actor and then back again to the host that initiated the call.
    let host = Host::new();

    let fs_binding_name = "fs_host_error_test_binding".to_string();

    host.add_native_capability(wascc_host::NativeCapability::from_instance(
        dummy_fs_provider::DummyFsProvider::new(),
        Some(fs_binding_name.clone()),
    )?)?;

    let actor = wascc_host::Actor::from_file("./tests/resources/dummy-actor/dummy_actor.wasm")?;

    host.add_actor(actor)?;
    host.set_binding(
        "MD3U6BFGA5LT7VUQK77247Z27XF3NBCSHXTFSZIIVLG5NYVK275I4VQX",
        "wascc:blobstore",
        Some(fs_binding_name.clone()),
        HashMap::new(),
    )?;

    let config = CapabilityConfiguration {
        module: "actor-init".to_string(),
        values: HashMap::new(),
    };
    let buf = wascc_codec::serialize(config).unwrap();

    let mut num_tries = 10;
    let expected_error = loop {
        //Expects the actor to trigger a call to a provider that will result in an error.
        //If it doesn't trigger an error this call will fail in a panic.
        let err = host
            .call_actor(
                "MD3U6BFGA5LT7VUQK77247Z27XF3NBCSHXTFSZIIVLG5NYVK275I4VQX",
                wascc_codec::core::OP_INITIALIZE,
                &buf,
            )
            .expect_err("Actor did not return an error as expected.");

        match err.kind() {
            wascc_host::errors::ErrorKind::IO(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if num_tries == 0 {
                    panic!("Could not get cal through to actor.");
                } else {
                    num_tries -= 1;
                    println!("looping {}", num_tries);
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
            _ => {
                break err.to_string();
            }
        }
    };

    let expected_end_str = "dummy_container_removal: THIS IS THE WAY";
    assert!(
        expected_error.ends_with(expected_end_str),
        "Error message received does not end with '{}'. Got this error instead: <{}>",
        expected_end_str,
        expected_error
    );

    host.shutdown()?;
    Ok(())
}
