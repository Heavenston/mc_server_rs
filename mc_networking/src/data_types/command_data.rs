use crate::data_types::VarInt;
use crate::data_types::encoder::PacketEncoder;

use std::rc::Rc;

pub trait Node {
    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8>;
}

#[derive(Clone)]
pub struct RootNode {
    pub is_executable: bool,
    pub children_nodes: Vec<Rc<dyn Node>>,
    pub redirect_node: Option<Rc<dyn Node>>,
}
impl Node for RootNode {
    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8> {
        let mut encoder = PacketEncoder::new();

        encoder.write_u8(0u8 // Node Type
            | (0x04 * self.is_executable as u8)
            | (0x08 * self.redirect_node.is_some() as u8));

        encoder.write_varint(self.children_nodes.len() as VarInt);
        for child in self.children_nodes.iter() {
            encoder.write_varint(graph_encoder.get_node(child));
        }

        if let Some(redirect_node) = self.redirect_node.as_ref() {
            encoder.write_varint(graph_encoder.get_node(redirect_node));
        }

        encoder.consume()
    }
}

#[derive(Clone)]
pub struct LiteralNode {
    pub is_executable: bool,
    pub children_nodes: Vec<Rc<dyn Node>>,
    pub redirect_node: Option<Rc<dyn Node>>,
    pub name: String,
}
impl Node for LiteralNode {
    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8> {
        let mut encoder = PacketEncoder::new();

        encoder.write_u8(1u8 // Node Type
            | (0x04 * self.is_executable as u8)
            | (0x08 * self.redirect_node.is_some() as u8));

        encoder.write_varint(self.children_nodes.len() as VarInt);
        for child in self.children_nodes.iter() {
            encoder.write_varint(graph_encoder.get_node(child));
        }

        if let Some(redirect_node) = self.redirect_node.as_ref() {
            encoder.write_varint(graph_encoder.get_node(redirect_node));
        }
        encoder.write_string(&self.name);

        encoder.consume()
    }
}

#[derive(Clone)]
pub struct ArgumentNode {
    pub is_executable: bool,
    pub children_nodes: Vec<Rc<dyn Node>>,
    pub redirect_node: Option<Rc<dyn Node>>,
    pub name: String,
    pub parser: String,
    /// Content depends on parser: https://wiki.vg/Command_Data#Parsers
    pub properties: Vec<u8>,
    pub suggestions_type: Option<String>,
}
impl Node for ArgumentNode {
    fn encode(&self, graph_encoder: &mut GraphEncoder) -> Vec<u8> {
        let mut encoder = PacketEncoder::new();

        encoder.write_u8(2u8 // Node Type
            | (0x04 * self.is_executable as u8)
            | (0x08 * self.redirect_node.is_some() as u8)
            | (0x10 * self.suggestions_type.is_some() as u8));

        encoder.write_varint(self.children_nodes.len() as VarInt);
        for child in self.children_nodes.iter() {
            encoder.write_varint(graph_encoder.get_node(child));
        }

        if let Some(redirect_node) = self.redirect_node.as_ref() {
            encoder.write_varint(graph_encoder.get_node(redirect_node));
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
    nodes: Vec<Rc<dyn Node>>,
}
impl GraphEncoder {
    pub fn new() -> Self {
        Self {
            nodes: vec![]
        }
    }

    /// Adds a node to the node list and returns its index
    pub fn add_node(&mut self, node: Rc<dyn Node>) -> i32 {
        self.nodes.push(node);
        self.nodes.len() as i32 - 1
    }
    /// Get the Index at which a node is, return -1 if not found
    pub fn get_node_index(&self, node: &Rc<dyn Node>) -> i32 {
        self.nodes.iter().enumerate()
            .find(|(_, c_node)| Rc::ptr_eq(c_node, &node))
            .map(|found| found.0 as i32)
            .unwrap_or(-1)
    }
    /// Get the index of a node, adds it if it is not found
    pub fn get_node(&mut self, node: &Rc<dyn Node>) -> i32 {
        match self.get_node_index(node) {
            -1 => self.add_node(node.clone()),
            index => index
        }
    }

    pub fn encode(mut self) -> Vec<Vec<u8>> {
        let mut array = vec![];
        let nodes = self.nodes.clone();
        for node in nodes {
            array.push(node.encode(&mut self));
        }
        array
    }
}