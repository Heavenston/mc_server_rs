use crate::data_types::{encoder::PacketEncoder, Identifier, VarInt};

use std::sync::Arc;

pub trait Node: Send + Sync {
    fn name(&self) -> String;
    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8>;
}

#[derive(Clone)]
pub struct RootNode {
    pub is_executable: bool,
    pub children_nodes: Vec<Arc<dyn Node>>,
    pub redirect_node: Option<Arc<dyn Node>>,
}
impl Node for RootNode {
    fn name(&self) -> String {
        "root".to_string()
    }

    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8> {
        let mut encoder = PacketEncoder::new();

        encoder.write_u8(
            0u8 // Node Type
            | (0x04 * self.is_executable as u8)
            | (0x08 * self.redirect_node.is_some() as u8),
        );

        encoder.write_varint(self.children_nodes.len() as VarInt);
        for child in self.children_nodes.iter() {
            encoder.write_varint(graph_encoder.add_node(child));
        }

        if let Some(redirect_node) = self.redirect_node.as_ref() {
            encoder.write_varint(graph_encoder.add_node(redirect_node));
        }

        encoder.consume()
    }
}

#[derive(Clone)]
pub struct LiteralNode {
    pub is_executable: bool,
    pub children_nodes: Vec<Arc<dyn Node>>,
    pub redirect_node: Option<Arc<dyn Node>>,
    pub name: String,
}
impl Node for LiteralNode {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8> {
        let mut encoder = PacketEncoder::new();

        encoder.write_u8(
            1u8 // Node Type
            | (0x04 * self.is_executable as u8)
            | (0x08 * self.redirect_node.is_some() as u8),
        );

        encoder.write_varint(self.children_nodes.len() as VarInt);
        for child in self.children_nodes.iter() {
            encoder.write_varint(graph_encoder.add_node(child));
        }

        if let Some(redirect_node) = self.redirect_node.as_ref() {
            encoder.write_varint(graph_encoder.add_node(redirect_node));
        }
        encoder.write_string(&self.name);

        encoder.consume()
    }
}

#[derive(Clone)]
pub struct ArgumentNode {
    pub is_executable: bool,
    pub children_nodes: Vec<Arc<dyn Node>>,
    pub redirect_node: Option<Arc<dyn Node>>,
    pub name: String,
    /// All parsers can be found here: https://wiki.vg/Command_Data#Parsers
    pub parser: Identifier,
    /// Content depends on parser: https://wiki.vg/Command_Data#Parsers
    pub properties: Vec<u8>,
    pub suggestions_type: Option<String>,
}
impl Node for ArgumentNode {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8> {
        let mut encoder = PacketEncoder::new();

        encoder.write_u8(
            2u8 // Node Type
            | (0x04 * self.is_executable as u8)
            | (0x08 * self.redirect_node.is_some() as u8)
            | (0x10 * self.suggestions_type.is_some() as u8),
        );

        encoder.write_varint(self.children_nodes.len() as VarInt);
        for child in self.children_nodes.iter() {
            encoder.write_varint(graph_encoder.add_node(child));
        }

        if let Some(redirect_node) = self.redirect_node.as_ref() {
            encoder.write_varint(graph_encoder.add_node(redirect_node));
        }

        encoder.write_string(&self.name);
        encoder.write_string(&self.parser);
        encoder.write_bytes(&self.properties);
        if let Some(suggestions_type) = self.suggestions_type.as_ref() {
            encoder.write_string(suggestions_type);
        }

        encoder.consume()
    }
}

#[derive(Clone)]
pub struct GraphEncoder {
    nodes: Vec<Arc<dyn Node>>,
    encoded: Vec<Vec<u8>>,
}
impl GraphEncoder {
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            encoded: vec![],
        }
    }

    /// Get the Index at which a node is, return -1 if not found
    pub fn get_node_index(&self, node: &Arc<dyn Node>) -> i32 {
        self.nodes
            .iter()
            .enumerate()
            .find(|(_, c_node)| Arc::ptr_eq(c_node, &node))
            .map(|found| found.0 as i32)
            .unwrap_or(-1)
    }
    /// Get the index of a node, adds it if it is not found
    pub fn add_node(&mut self, node: &Arc<dyn Node>) -> i32 {
        match self.get_node_index(node) {
            -1 => {
                let index = self.nodes.len();
                self.nodes.push(node.clone());
                self.encoded.push(vec![]);
                let encoded = node.encode(self);
                self.encoded[index] = encoded;
                index as i32
            }
            index => index,
        }
    }

    pub fn encode(self) -> Vec<Vec<u8>> {
        self.encoded
    }
}
