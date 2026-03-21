#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

pub enum ExecutionPath {
    BedrockDirect,
    DirectXrpl,
}

pub enum AdapterError {
    Unsupported,
    SubmissionFailed,
    Timeout,
}

pub struct SwapRequest {
    pub amount_in: u64,
    pub min_amount_out: u64,
    pub zero_for_one: bool,
}

pub struct SwapReceipt {
    pub path: ExecutionPath,
    pub amount_out: u64,
    pub tx_hash: [u8; 32],
}

pub trait ExecutionAdapter {
    fn supports_path(&self, path: ExecutionPath) -> bool;
    fn submit_swap(&self, req: &SwapRequest) -> Result<SwapReceipt, AdapterError>;
}

pub struct DualPathAdapter {
    pub prefer_bedrock: bool,
    pub bedrock_available: bool,
    pub xrpl_available: bool,
}

impl DualPathAdapter {
    pub fn choose_path(&self) -> Result<ExecutionPath, AdapterError> {
        if self.prefer_bedrock && self.bedrock_available {
            return Ok(ExecutionPath::BedrockDirect);
        }
        if self.xrpl_available {
            return Ok(ExecutionPath::DirectXrpl);
        }
        if self.bedrock_available {
            return Ok(ExecutionPath::BedrockDirect);
        }
        Err(AdapterError::Unsupported)
    }

    pub fn execute_with_fallback(&self, req: &SwapRequest) -> Result<SwapReceipt, AdapterError> {
        match self.choose_path()? {
            ExecutionPath::BedrockDirect => {
                match self.submit_bedrock(req) {
                    Ok(r) => Ok(r),
                    Err(_) => self.submit_xrpl(req),
                }
            }
            ExecutionPath::DirectXrpl => self.submit_xrpl(req),
        }
    }

    fn submit_bedrock(&self, req: &SwapRequest) -> Result<SwapReceipt, AdapterError> {
        if !self.bedrock_available {
            return Err(AdapterError::Unsupported);
        }
        // TODO: wire to Bedrock call transport.
        Ok(SwapReceipt {
            path: ExecutionPath::BedrockDirect,
            amount_out: req.amount_in.saturating_sub(1),
            tx_hash: [1u8; 32],
        })
    }

    fn submit_xrpl(&self, req: &SwapRequest) -> Result<SwapReceipt, AdapterError> {
        if !self.xrpl_available {
            return Err(AdapterError::Unsupported);
        }
        // TODO: wire to direct XRPL signed transaction path.
        Ok(SwapReceipt {
            path: ExecutionPath::DirectXrpl,
            amount_out: req.amount_in.saturating_sub(2),
            tx_hash: [2u8; 32],
        })
    }
}
