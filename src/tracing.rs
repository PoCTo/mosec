// Copyright 2022 MOSEC Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::borrow::Cow;
use std::env;

use fastrace::collector::Config;
use fastrace_opentelemetry::OpenTelemetryReporter;
use log::info;
use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;

const DEFAULT_SERVICE_NAME: &str = "mosec";

/// Initialize the fastrace reporter with an OTLP gRPC exporter.
///
/// Reads configuration from standard OpenTelemetry environment variables:
/// - `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` (default: `http://localhost:4317`)
/// - `OTEL_SERVICE_NAME` (default: `mosec`)
///
/// If `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` is not set, no reporter is initialized
/// and all fastrace spans become no-ops (zero overhead).
pub fn init_tracing() {
    let endpoint = match env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") {
        Ok(val) if !val.is_empty() => val,
        _ => {
            info!("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT not set, tracing disabled");
            return;
        }
    };

    let service_name = env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| DEFAULT_SERVICE_NAME.to_string());

    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.clone())
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_timeout(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT)
        .build()
        .expect("failed to initialize OTLP span exporter");

    let reporter = OpenTelemetryReporter::new(
        exporter,
        Cow::Owned(
            Resource::builder()
                .with_attributes([KeyValue::new("service.name", service_name.clone())])
                .build(),
        ),
        InstrumentationScope::builder("mosec")
            .with_version(env!("CARGO_PKG_VERSION"))
            .build(),
    );

    fastrace::set_reporter(reporter, Config::default());
    info!(endpoint:%, service_name:%; "tracing initialized with OTLP exporter");
}

#[cfg(test)]
mod tests {
    use fastrace::prelude::*;

    #[test]
    fn decode_w3c_traceparent_roundtrip() {
        let traceparent = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        let ctx = SpanContext::decode_w3c_traceparent(traceparent).unwrap();
        assert_eq!(ctx.trace_id, TraceId(0x0af7651916cd43dd8448eb211c80319c));
        assert_eq!(ctx.span_id, SpanId(0xb7ad6b7169203331));
        assert!(ctx.sampled);
    }

    #[test]
    fn decode_w3c_traceparent_not_sampled() {
        let traceparent = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-00";
        let ctx = SpanContext::decode_w3c_traceparent(traceparent).unwrap();
        assert!(!ctx.sampled);
    }

    #[test]
    fn decode_w3c_traceparent_invalid_returns_none() {
        assert!(SpanContext::decode_w3c_traceparent("invalid").is_none());
        assert!(SpanContext::decode_w3c_traceparent("").is_none());
        assert!(SpanContext::decode_w3c_traceparent("00-bad-data-01").is_none());
    }
}
