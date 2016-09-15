// Copyright (C) 2016 Pietro Albini
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::time::Duration;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::fs;

use chan;
use hyper::client as hyper;
use hyper::method::Method;

use app::FisherOptions;
use hooks::{self, Hooks};
use jobs::Job;
use web::WebApi;
use requests::Request;
use processor::{ProcessorInput, HealthDetails};
use providers::{Provider, testing};
use utils;


pub fn dummy_request() -> Request {
    Request {
        headers: HashMap::new(),
        params: HashMap::new(),
        source: SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 80
        ),
        body: String::new(),
    }
}


pub fn testing_provider() -> Provider {
    Provider::new(
        "Testing".to_string(),
        testing::check_config,
        testing::request_type,
        testing::validate,
        testing::env,
    )
}


macro_rules! create_hook {
    ($tempdir:expr, $name:expr, $( $line:expr ),* ) => {{
        use std::fs;
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;

        let mut hook_path = $tempdir.clone();
        hook_path.push($name);

        let mut hook = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o755)
            .open(&hook_path)
            .unwrap();

        let res = write!(hook, "{}", concat!(
            $(
                $line, "\n",
            )*
        ));
        res.unwrap();
    }};
}


pub fn sample_hooks() -> PathBuf {
    // Create a sample directory with some hooks
    let tempdir = utils::create_temp_dir().unwrap();

    create_hook!(tempdir, "example.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"echo "Hello world""#
    );

    create_hook!(tempdir, "failing.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"exit 1"#
    );

    create_hook!(tempdir, "jobs-details.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"b="${FISHER_TESTING_ENV}""#,
        r#"echo "executed" > "${b}/executed""#,
        r#"env > "${b}/env""#,
        r#"pwd > "${b}/pwd""#,
        r#"cat "${FISHER_REQUEST_BODY}" > "${b}/request_body""#
    );

    tempdir
}


pub struct WebApiInstance<'a> {
    inst: WebApi<'a>,

    url: String,
    client: hyper::Client,
    input_recv: chan::Receiver<ProcessorInput>,
}

impl<'a> WebApiInstance<'a> {

    pub fn new(hooks: &'a Hooks, health: bool) -> Self {
        // Create a new instance of WebApi
        let mut inst = WebApi::new(hooks);

        // Create the input channel
        let (input_send, input_recv) = chan::async();

        // Set the options
        let options = FisherOptions {
            bind: "127.0.0.1:0".to_string(),
            enable_health: health,

            .. FisherOptions::defaults()
        };

        // Start the web server
        let addr = inst.listen(&options, input_send).unwrap();

        // Create the HTTP client
        let url = format!("http://{}", addr);
        let client = hyper::Client::new();

        WebApiInstance {
            inst: inst,

            url: url,
            client: client,
            input_recv: input_recv,
        }
    }

    pub fn request(&mut self, method: Method, url: &str)
                   -> hyper::RequestBuilder {
        // Create the HTTP request
        self.client.request(method, &format!("{}{}", self.url, url))
    }

    pub fn processor_input(&self) -> Option<ProcessorInput> {
        let input_recv = &self.input_recv;

        // This returns Some only if there is something right now
        chan_select! {
            default => {
                return None;
            },
            input_recv.recv() -> input => {
                return Some(input.unwrap());
            },
        };
    }

    pub fn next_health(&self, details: HealthDetails) -> NextHealthCheck {
        let input_chan = self.input_recv.clone();
        let (result_send, result_recv) = chan::async();

        ::std::thread::spawn(move || {
            let input = input_chan.recv().unwrap();

            if let ProcessorInput::HealthStatus(ref sender) = input {
                // Send the HealthDetails we want
                sender.send(details);

                // Everything was OK
                result_send.send(None);
            } else {
                result_send.send(Some(
                    "Wrong kind of ProcessorInput received!".to_string()
                ));
            }
        });

        NextHealthCheck::new(result_recv)
    }

    pub fn stop(&mut self) -> bool {
        self.inst.stop()
    }
}


pub struct TestingEnv {
    hooks: Hooks,
    remove_dirs: Vec<String>,
}

impl TestingEnv {

    pub fn new() -> Self {
        let hooks_dir = sample_hooks().to_str().unwrap().to_string();

        TestingEnv {
            hooks: hooks::collect(&hooks_dir).unwrap(),
            remove_dirs: vec![hooks_dir],
        }
    }

    // CLEANUP

    pub fn delete_also(&mut self, path: &str) {
        self.remove_dirs.push(path.to_string());
    }

    pub fn cleanup(&self) {
        // Remove all the directories
        for dir in &self.remove_dirs {
            let _ = fs::remove_dir_all(dir);
        }
    }

    // JOBS UTILITIES

    pub fn create_job(&self, hook_name: &str, req: Request) -> Job {
        let hook = self.hooks.get(&hook_name.to_string()).unwrap();
        let (_, provider) = hook.validate(&req);

        Job::new(hook.clone(), provider, req)
    }

    // WEB TESTING

    pub fn start_web(&self, health: bool) -> WebApiInstance {
        WebApiInstance::new(&self.hooks, health)
    }
}


pub struct NextHealthCheck {
    result_recv: chan::Receiver<Option<String>>,
}

impl NextHealthCheck {

    fn new(result_recv: chan::Receiver<Option<String>>) -> Self {
        NextHealthCheck {
            result_recv: result_recv,
        }
    }

    pub fn check(&self) {
        let result_recv = &self.result_recv;

        let timeout = chan::after(Duration::from_secs(5));

        chan_select! {
            timeout.recv() => {
                panic!("No ProcessorInput received!");
            },
            result_recv.recv() -> result => {
                // Forward panics
                if let Some(message) = result.unwrap() {
                    panic!(message);
                }
            },
        };
    }
}
