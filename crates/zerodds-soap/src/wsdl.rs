// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WSDL 1.1 + 2.0 generation from IDL service defs.
//!
//! WSDL 1.1: `http://schemas.xmlsoap.org/wsdl/`
//! WSDL 2.0: `http://www.w3.org/ns/wsdl`
//!
//! We generate spec-conformant `<types>`/`<message>`/`<portType>`/
//! `<binding>`/`<service>` (1.1) or `<types>`/`<interface>`/
//! `<binding>`/`<service>` (2.0).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// WSDL version selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsdlVersion {
    /// WSDL 1.1 (`http://schemas.xmlsoap.org/wsdl/`).
    V11,
    /// WSDL 2.0 (`http://www.w3.org/ns/wsdl`).
    V20,
}

/// Operation of a WSDL interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Operation name.
    pub name: String,
    /// Input message element (XSD type name).
    pub input_type: String,
    /// Output message element (XSD type name).
    pub output_type: String,
}

/// WSDL generator builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsdlGenerator {
    service_name: String,
    target_namespace: String,
    endpoint_url: String,
    operations: Vec<Operation>,
    version: WsdlVersion,
}

impl WsdlGenerator {
    /// Constructor.
    #[must_use]
    pub fn new(service_name: &str, namespace: &str, endpoint: &str, version: WsdlVersion) -> Self {
        Self {
            service_name: service_name.into(),
            target_namespace: namespace.into(),
            endpoint_url: endpoint.into(),
            operations: Vec::new(),
            version,
        }
    }

    /// Add an operation.
    #[must_use]
    pub fn operation(mut self, op: Operation) -> Self {
        self.operations.push(op);
        self
    }

    /// Render method (per the version switch).
    #[must_use]
    pub fn render(&self) -> String {
        match self.version {
            WsdlVersion::V11 => self.render_v11(),
            WsdlVersion::V20 => self.render_v20(),
        }
    }

    fn render_v11(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\"?>\n");
        out.push_str(&format!(
            "<definitions name=\"{}\" targetNamespace=\"{}\" \
xmlns=\"http://schemas.xmlsoap.org/wsdl/\" \
xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:tns=\"{}\" \
xmlns:soap=\"http://schemas.xmlsoap.org/wsdl/soap/\">\n",
            self.service_name, self.target_namespace, self.target_namespace
        ));
        // Messages.
        for op in &self.operations {
            out.push_str(&format!(
                "  <message name=\"{}Request\"><part name=\"body\" element=\"tns:{}\"/></message>\n",
                op.name, op.input_type
            ));
            out.push_str(&format!(
                "  <message name=\"{}Response\"><part name=\"body\" element=\"tns:{}\"/></message>\n",
                op.name, op.output_type
            ));
        }
        // PortType.
        out.push_str(&format!(
            "  <portType name=\"{}PortType\">\n",
            self.service_name
        ));
        for op in &self.operations {
            out.push_str(&format!(
                "    <operation name=\"{0}\">\n      <input message=\"tns:{0}Request\"/>\n      <output message=\"tns:{0}Response\"/>\n    </operation>\n",
                op.name
            ));
        }
        out.push_str("  </portType>\n");
        // Binding.
        out.push_str(&format!(
            "  <binding name=\"{0}SoapBinding\" type=\"tns:{0}PortType\">\n    <soap:binding style=\"document\" transport=\"http://schemas.xmlsoap.org/soap/http\"/>\n",
            self.service_name
        ));
        for op in &self.operations {
            out.push_str(&format!(
                "    <operation name=\"{}\"><soap:operation soapAction=\"{}/{}\"/><input><soap:body use=\"literal\"/></input><output><soap:body use=\"literal\"/></output></operation>\n",
                op.name, self.target_namespace, op.name
            ));
        }
        out.push_str("  </binding>\n");
        // Service.
        out.push_str(&format!(
            "  <service name=\"{0}\">\n    <port name=\"{0}Port\" binding=\"tns:{0}SoapBinding\">\n      <soap:address location=\"{1}\"/>\n    </port>\n  </service>\n",
            self.service_name, self.endpoint_url
        ));
        out.push_str("</definitions>\n");
        out
    }

    fn render_v20(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\"?>\n");
        out.push_str(&format!(
            "<description xmlns=\"http://www.w3.org/ns/wsdl\" \
targetNamespace=\"{}\" \
xmlns:tns=\"{}\" \
xmlns:wsoap=\"http://www.w3.org/ns/wsdl/soap\">\n",
            self.target_namespace, self.target_namespace
        ));
        // Interface.
        out.push_str(&format!(
            "  <interface name=\"{}Interface\">\n",
            self.service_name
        ));
        for op in &self.operations {
            out.push_str(&format!(
                "    <operation name=\"{0}\" pattern=\"http://www.w3.org/ns/wsdl/in-out\">\n      <input element=\"tns:{1}\"/>\n      <output element=\"tns:{2}\"/>\n    </operation>\n",
                op.name, op.input_type, op.output_type
            ));
        }
        out.push_str("  </interface>\n");
        // Binding.
        out.push_str(&format!(
            "  <binding name=\"{0}SoapBinding\" interface=\"tns:{0}Interface\" type=\"http://www.w3.org/ns/wsdl/soap\" wsoap:protocol=\"http://www.w3.org/2003/05/soap/bindings/HTTP/\"/>\n",
            self.service_name
        ));
        // Service.
        out.push_str(&format!(
            "  <service name=\"{0}\" interface=\"tns:{0}Interface\">\n    <endpoint name=\"{0}Endpoint\" binding=\"tns:{0}SoapBinding\" address=\"{1}\"/>\n  </service>\n",
            self.service_name, self.endpoint_url
        ));
        out.push_str("</description>\n");
        out
    }

    /// Number of configured operations.
    #[must_use]
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn echo_op() -> Operation {
        Operation {
            name: "Echo".into(),
            input_type: "EchoRequest".into(),
            output_type: "EchoResponse".into(),
        }
    }

    #[test]
    fn wsdl_11_emits_definitions_root() {
        let g = WsdlGenerator::new(
            "TraderService",
            "http://demo/trader",
            "http://localhost/svc",
            WsdlVersion::V11,
        )
        .operation(echo_op());
        let xml = g.render();
        assert!(xml.contains("<definitions name=\"TraderService\""));
        assert!(xml.contains("<portType name=\"TraderServicePortType\""));
        assert!(xml.contains("<message name=\"EchoRequest\""));
        assert!(xml.contains("<service name=\"TraderService\""));
        assert!(xml.contains("schemas.xmlsoap.org/wsdl/"));
    }

    #[test]
    fn wsdl_20_emits_description_root() {
        let g = WsdlGenerator::new(
            "TraderService",
            "http://demo/trader",
            "http://localhost/svc",
            WsdlVersion::V20,
        )
        .operation(echo_op());
        let xml = g.render();
        assert!(xml.contains("<description xmlns=\"http://www.w3.org/ns/wsdl\""));
        assert!(xml.contains("<interface name=\"TraderServiceInterface\">"));
        assert!(xml.contains("<endpoint name=\"TraderServiceEndpoint\""));
    }

    #[test]
    fn wsdl_11_includes_all_operation_messages() {
        let g = WsdlGenerator::new("S", "http://demo/", "http://demo/svc", WsdlVersion::V11)
            .operation(echo_op())
            .operation(Operation {
                name: "Ping".into(),
                input_type: "PingRequest".into(),
                output_type: "PingResponse".into(),
            });
        let xml = g.render();
        assert!(xml.contains("EchoRequest"));
        assert!(xml.contains("EchoResponse"));
        assert!(xml.contains("PingRequest"));
        assert!(xml.contains("PingResponse"));
    }

    #[test]
    fn wsdl_11_binding_uses_soap_http_transport() {
        let g = WsdlGenerator::new("S", "http://demo/", "http://demo/svc", WsdlVersion::V11)
            .operation(echo_op());
        let xml = g.render();
        assert!(xml.contains("transport=\"http://schemas.xmlsoap.org/soap/http\""));
    }

    #[test]
    fn wsdl_20_binding_targets_soap_protocol() {
        let g = WsdlGenerator::new("S", "http://demo/", "http://demo/svc", WsdlVersion::V20)
            .operation(echo_op());
        let xml = g.render();
        assert!(xml.contains("wsoap:protocol="));
        assert!(xml.contains("soap/bindings/HTTP"));
    }

    #[test]
    fn endpoint_address_is_included() {
        let g = WsdlGenerator::new(
            "S",
            "http://demo/",
            "http://example.org:8080/api",
            WsdlVersion::V11,
        )
        .operation(echo_op());
        let xml = g.render();
        assert!(xml.contains("location=\"http://example.org:8080/api\""));
    }
}
