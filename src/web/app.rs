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

use std::sync::Arc;
use std::net::SocketAddr;

use chan;
use tiny_http::Method;

use errors::FisherResult;
use app::FisherOptions;
use hooks::Hooks;
use processor::ProcessorInput;
use web::http::HttpServer;
use web::api::WebApi;


pub struct WebApp {
    server: Option<HttpServer<WebApi>>,
}

impl WebApp {

    pub fn new() -> Self {
        WebApp {
            server: None,
        }
    }

    pub fn listen(&mut self, hooks: Arc<Hooks>, options: &FisherOptions,
                  input: chan::Sender<ProcessorInput>)
                 -> FisherResult<SocketAddr> {
        // Create the web api
        let api = WebApi::new(input, hooks, options.enable_health);

        // Create the HTTP server
        let mut server = HttpServer::new(api, options.behind_proxies);
        server.add_route(
            Method::Get, "/health",
            Box::new(WebApi::get_health)
        );
        server.add_route(
            Method::Get, "/hook/?",
            Box::new(WebApi::process_hook)
        );
        server.add_route(
            Method::Post, "/hook/?",
            Box::new(WebApi::process_hook)
        );

        let socket = try!(server.listen(&options.bind));

        self.server = Some(server);
        Ok(socket)
    }

    pub fn stop(&mut self) -> bool {
        if let Some(ref mut server) = self.server {
            server.stop()
        } else {
            false
        }
    }
}


#[cfg(test)]
mod tests {
    use std::io::Read;

    use hyper::status::StatusCode;
    use hyper::method::Method;
    use hyper::header::Headers;
    use rustc_serialize::json::Json;

    use utils::testing::*;
    use processor::{HealthDetails, ProcessorInput};


    #[test]
    fn test_startup() {
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, None);

        // Test if the Web API is working fine
        let res = inst.request(Method::Get, "/").send().unwrap();
        assert_eq!(res.status, StatusCode::NotFound);

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_hook_call() {
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, None);

        // It shouldn't be possible to call a non-existing hook
        let res = inst.request(Method::Get, "/hook/invalid")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::NotFound);
        assert!(inst.processor_input().is_none());

        // Call the example hook without authorization
        let res = inst.request(Method::Get, "/hook/example?secret=invalid")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);
        assert!(inst.processor_input().is_none());

        // Call the example hook with authorization
        let res = inst.request(Method::Get, "/hook/example?secret=testing")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Assert a job is queued
        let input = inst.processor_input();
        assert!(input.is_some());

        // Assert the right job is queued
        if let ProcessorInput::Job(job) = input.unwrap() {
            assert_eq!(job.hook_name(), "example");
        } else {
            panic!("Wrong processor input received");
        }

        // Call the example hook simulating a Ping
        let res = inst.request(Method::Get, "/hook/example?request_type=ping")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Even if the last request succeded, there shouldn't be any job
        assert!(inst.processor_input().is_none());

        // Try to call an internal hook (in this case with the Status provider)
        let res = inst.request(Method::Get, concat!(
            "/hook/status-example",
            "?event=job_completed",
            "&hook_name=trigger-status",
            "&exit_code=0",
            "&signal=0",
        )).send().unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);

        // Even if the last request succeded, there shouldn't be any job
        assert!(inst.processor_input().is_none());

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_health_disabled() {
        // Create the instance with disabled health status
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(false, None);

        // It shouldn't be possible to get the health status
        let res = inst.request(Method::Get, "/health").send().unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_health_enabled() {
        // Create the instance with enabled health status
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, None);

        let check_after = inst.next_health(HealthDetails {
            queue_size: 1,
            active_jobs: 2,
        });

        // Assert the request is OK
        let mut res = inst.request(Method::Get, "/health").send().unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Decode the output
        let mut content = String::new();
        res.read_to_string(&mut content).unwrap();
        let data = Json::from_str(&content).unwrap();
        let data_obj = data.as_object().unwrap();

        // Check the content of the returned JSON
        let result = data_obj.get("result").unwrap().as_object().unwrap();
        assert_eq!(
            result.get("queue_size").unwrap().as_u64().unwrap(),
            1 as u64
        );
        assert_eq!(
            result.get("active_jobs").unwrap().as_u64().unwrap(),
            2 as u64
        );

        // Check if there were any problems into the next_health thread
        check_after.check();

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_behind_proxy() {
        // Create a new instance behind a proxy
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, Some(1));

        // Call the example hook without a proxy
        let res = inst.request(Method::Get, "/hook/example?ip=127.1.1.1")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::BadRequest);
        assert!(inst.processor_input().is_none());

        // Build the headers for a proxy
        let mut headers = Headers::new();
        headers.set_raw("X-Forwarded-For", vec![b"127.1.1.1".to_vec()]);

        // Make an example request
        let res = inst.request(Method::Get, "/hook/example?ip=127.1.1.1")
                      .headers(headers).send().unwrap();

        // The hook should be queued
        assert_eq!(res.status, StatusCode::Ok);
        assert!(inst.processor_input().is_some());

        inst.stop();
        testing_env.cleanup();
    }
}