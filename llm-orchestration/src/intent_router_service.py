import grpc
from concurrent import futures
import json
import re
import logging
from datetime import datetime
import subprocess
import sys
import os
from typing import Optional, Dict, Any

# Import your generated proto files
import intent_router_pb2
import intent_router_pb2_grpc

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class IntentRouterServicer(intent_router_pb2_grpc.IntentRouterServicer):
    """gRPC service for local LLM-based intent classification"""
    
    def __init__(self, model: str = "llama3.2:3b", use_ollama: bool = True):
        """
        Initialize the intent router with a local LLM.
        
        Args:
            model: Model name to use (e.g., "llama2", "neural-chat", "mistral")
            use_ollama: Whether to use Ollama CLI or direct inference
        """
        self.llm_model = model
        self.use_ollama = use_ollama
        self.llm_available = self._check_llm_available()
        
        self.prompt_template = """TASK: Classify the user query into a JSON format. RESPOND ONLY WITH JSON, NOTHING ELSE.

USER QUERY: {user_query}

RETURN ONLY THIS JSON FORMAT (no explanation, no extra text):
{{"action": "analyze_risk|execute_strategy|check_position|get_price", "scope": "portfolio|specific_asset|specific_pool", "confidence": 0.0-1.0, "parameters": {{}}}}

Examples:
- "analyze my portfolio" → {{"action":"analyze_risk","scope":"portfolio","confidence":0.95,"parameters":{{}}}}
- "XRP price" → {{"action":"get_price","scope":"specific_asset","confidence":0.9,"parameters":{{"asset":"XRP"}}}}
- "hedge strategy" → {{"action":"execute_strategy","scope":"portfolio","confidence":0.85,"parameters":{{"strategy":"conservative"}}}}

REMEMBER: RESPOND ONLY WITH JSON. NO WORDS BEFORE OR AFTER."""

    def _check_llm_available(self) -> bool:
        """Check if local LLM is available"""
        try:
            if self.use_ollama:
                result = subprocess.run(
                    ["ollama", "list"],
                    capture_output=True,
                    text=True,
                    timeout=2
                )
                return result.returncode == 0
        except Exception as e:
            logger.warning(f"LLM availability check failed: {e}")
        return False

    def _call_local_llm(self, prompt: str) -> Optional[str]:
        """
        Call local LLM (Ollama or llama.cpp).
        
        Args:
            prompt: The prompt to send to the LLM
            
        Returns:
            LLM output string or None if failed
        """
        if not self.llm_available:
            logger.error("Local LLM not available")
            return None
            
        try:
            if self.use_ollama:
                # Using Ollama CLI (assumes `ollama serve` is running)
                # First call takes longer as model loads into VRAM
                env = os.environ.copy()
                env.setdefault("OLLAMA_HOST", "127.0.0.1:11434")
                result = subprocess.run(
                    ["ollama", "run", self.llm_model, prompt],
                    capture_output=True,
                    text=True,
                    timeout=60,  # 60s timeout (model loads into GPU memory first time)
                    env=env,
                )
                
                if result.returncode == 0:
                    return result.stdout.strip()
                else:
                    logger.error(f"Ollama error: {result.stderr}")
                    return None
            else:
                # Direct inference (llama.cpp or similar)
                # This would require additional setup
                logger.warning("Direct inference not yet implemented")
                return None
                
        except subprocess.TimeoutExpired:
            logger.error("LLM inference timeout (>10s)")
            return None
        except Exception as e:
            logger.error(f"LLM error: {e}")
            return None

    def _classify_with_keywords(self, query: str) -> Optional[Dict[str, Any]]:
        """
        Rule-based fallback classifier used when the local LLM is unavailable.
        Covers the most common query patterns with reasonable confidence.
        """
        q = query.lower()

        # execute / confirm intent
        if any(w in q for w in ["execute", "confirm", "do it", "proceed", "go ahead", "run it"]):
            return {"action": "execute_strategy", "scope": "portfolio", "confidence": 0.75, "parameters": {}}

        # price lookup intent
        if any(w in q for w in ["price", "worth", "how much", "value", "rate", "cost"]):
            scope = "specific_asset"
            params: Dict[str, Any] = {}
            for asset in ["xrp", "usd", "btc", "eth", "usdc", "usdt"]:
                if asset in q:
                    params["asset"] = asset.upper()
                    break
            return {"action": "get_price", "scope": scope, "confidence": 0.75, "parameters": params}

        # check position intent
        if any(w in q for w in ["check", "status", "show", "display", "view", "what is my"]):
            return {"action": "check_position", "scope": "portfolio", "confidence": 0.70, "parameters": {}}

        # analyze / risk / rebalance intent (broadest category — default)
        if any(w in q for w in [
            "rebalance", "analyze", "analyse", "risk", "portfolio", "hedge",
            "il", "impermanent", "loss", "sharpe", "var", "volatility",
            "strategy", "should i", "what should", "recommend", "advice",
        ]):
            return {"action": "analyze_risk", "scope": "portfolio", "confidence": 0.72, "parameters": {}}

        # Generic fallback — treat any unrecognised query as a portfolio analysis request
        return {"action": "analyze_risk", "scope": "portfolio", "confidence": 0.60, "parameters": {}}

    def _parse_intent_response(self, llm_output: str) -> Optional[Dict[str, Any]]:
        """
        Extract and validate JSON from LLM output.
        
        Args:
            llm_output: Raw output from LLM
            
        Returns:
            Parsed JSON dict or None if invalid
        """
        try:
            # Try to find JSON in output (LLM might add extra text)
            json_match = re.search(r'\{[\s\S]*\}', llm_output, re.DOTALL)
            
            if not json_match:
                logger.warning(f"No JSON found in LLM output: {llm_output}")
                return None
            
            parsed = json.loads(json_match.group())
            
            # Validate required fields - must have at least action and scope
            required_fields = ["action", "scope"]
            if all(field in parsed for field in required_fields):
                # Validate action values
                valid_actions = ["analyze_risk", "execute_strategy", "check_position", "get_price"]
                if parsed["action"] not in valid_actions:
                    logger.warning(f"Invalid action: {parsed['action']}")
                    return None
                
                # Validate scope values
                valid_scopes = ["portfolio", "specific_asset", "specific_pool"]
                if parsed["scope"] not in valid_scopes:
                    logger.warning(f"Invalid scope: {parsed['scope']}")
                    return None
                
                return parsed
            
            # If missing required fields, log what we got and try to infer
            logger.warning(f"Missing required fields in parsed JSON: {parsed}")
            logger.warning(f"Raw LLM output was: {llm_output}")
            return None
            
        except json.JSONDecodeError as e:
            logger.warning(f"Failed to parse LLM output as JSON: {e}")
            logger.warning(f"Raw output: {llm_output}")
            return None

    def ClassifyIntent(self, request, context):
        """
        Main RPC method to classify user intent.
        
        Args:
            request: IntentRequest with user_query
            context: gRPC context
            
        Returns:
            IntentResponse with classified intent
        """
        print(f"\n{'='*60}")
        print(f"📨 [Intent Router] Received query: '{request.user_query}'")
        print(f"{'='*60}")
        
        logger.info(f"Received query: {request.user_query}")
        
        # Build prompt with user query
        prompt = self.prompt_template.format(user_query=request.user_query)
        
        # Call local LLM
        print(f"🧠 [Intent Router] Calling local LLM (Ollama)...")
        llm_output = self._call_local_llm(prompt)
        
        if not llm_output:
            print(f"⚠️  [Intent Router] LLM unavailable — using keyword fallback")
            logger.warning("LLM inference returned None; falling back to keyword classifier")
            parsed = self._classify_with_keywords(request.user_query)
        else:
            print(f"📝 [Intent Router] LLM output:\n{llm_output}\n")

            # Parse and validate response
            print(f"🔍 [Intent Router] Parsing response...")
            parsed = self._parse_intent_response(llm_output)

            if not parsed:
                print(f"⚠️  [Intent Router] LLM parse failed — using keyword fallback")
                logger.warning("Failed to parse LLM response; falling back to keyword classifier")
                parsed = self._classify_with_keywords(request.user_query)
        
        print(f"✅ [Intent Router] Successfully classified:")
        print(f"   Action: {parsed.get('action')}")
        print(f"   Scope: {parsed.get('scope')}")
        print(f"   Confidence: {parsed.get('confidence')}")
        print(f"\n✅ [Intent Router] Returning response to Backend\n")
        
        # Convert parameters dict to repeated Parameter message
        parameters = [
            intent_router_pb2.Parameter(key=k, value=str(v))
            for k, v in parsed.items()
            if k not in ["action", "scope"]
        ]
        
        # Build response
        response = intent_router_pb2.IntentResponse(
            action=parsed.get("action", "unknown"),
            scope=parsed.get("scope", "unknown"),
            parameters=parameters,
            confidence=0.95,  # High confidence if validation passed
            is_valid=True
        )
        
        logger.info(
            f"Response: action={response.action}, scope={response.scope}, "
            f"confidence={response.confidence}"
        )
        return response


def serve(host: str = "[::]:50051", model: str = "llama3.2:3b"):
    """
    Start gRPC server.
    
    Args:
        host: Server address and port (default: all interfaces on 50051)
        model: LLM model to use
    """
    servicer = IntentRouterServicer(model=model, use_ollama=True)
    
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    intent_router_pb2_grpc.add_IntentRouterServicer_to_server(servicer, server)
    server.add_insecure_port(host)
    
    logger.info(f"Intent Router listening on {host}")
    logger.info(f"Using model: {model}")
    logger.info(f"LLM available: {servicer.llm_available}")
    
    server.start()
    
    try:
        server.wait_for_termination()
    except KeyboardInterrupt:
        logger.info("Shutting down server...")
        server.stop(0)


if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser(description="Intent Router gRPC Server")
    parser.add_argument(
        "--model",
        default="llama3.2:3b",
        help="LLM model to use (default: llama3.2:3b)"
    )
    parser.add_argument(
        "--host",
        default="[::]:50051",
        help="Server address and port (default: [::]:50051)"
    )
    
    args = parser.parse_args()
    
    serve(host=args.host, model=args.model)
