"""
Test client for Intent Router gRPC service.
Tests various user queries and displays results.
"""

import grpc
import sys
import os
from typing import List, Tuple

# Add src directory to path so we can import generated proto files
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'src'))

import intent_router_pb2
import intent_router_pb2_grpc


class IntentRouterClient:
    """Client for testing Intent Router service"""
    
    def __init__(self, host: str = "localhost", port: int = 50051):
        """
        Initialize client.
        
        Args:
            host: Server host (default: localhost)
            port: Server port (default: 50051)
        """
        self.channel = grpc.insecure_channel(f"{host}:{port}")
        self.stub = intent_router_pb2_grpc.IntentRouterStub(self.channel)

    def classify_query(self, query: str) -> intent_router_pb2.IntentResponse:
        """
        Classify a user query.
        
        Args:
            query: User's natural language query
            
        Returns:
            IntentResponse with classification results
        """
        request = intent_router_pb2.IntentRequest(user_query=query)
        response = self.stub.ClassifyIntent(request)
        return response

    def print_response(self, query: str, response: intent_router_pb2.IntentResponse):
        """Pretty print response"""
        print(f"\n{'='*70}")
        print(f"Query: {query}")
        print(f"{'='*70}")
        print(f"Action:     {response.action}")
        print(f"Scope:      {response.scope}")
        print(f"Confidence: {response.confidence:.2%}")
        print(f"Valid:      {response.is_valid}")
        
        if response.parameters:
            print(f"Parameters:")
            for param in response.parameters:
                print(f"  - {param.key}: {param.value}")
        
        if not response.is_valid:
            print(f"Error: {response.raw_llm_output}")

    def close(self):
        """Close connection"""
        self.channel.close()


def main():
    """Run test suite"""
    
    # Test queries from PROMPTS.md examples
    test_cases: List[Tuple[str, str]] = [
        # Format: (query, expected_action)
        ("Analyze my portfolio risk", "analyze_risk"),
        ("What's my XRP/USD position worth?", "check_position"),
        ("Execute the conservative hedge", "execute_strategy"),
        ("How much IL do I have?", "analyze_risk"),
        ("Check my current positions", "check_position"),
        ("What's the price of Bitcoin?", "get_price"),
        ("I want to execute strategy A", "execute_strategy"),
        ("Show me portfolio analysis", "analyze_risk"),
    ]
    
    try:
        client = IntentRouterClient(host="localhost", port=50051)
        print("Connected to Intent Router service")
        
        results = []
        for query, expected_action in test_cases:
            response = client.classify_query(query)
            client.print_response(query, response)
            
            results.append({
                "query": query,
                "action": response.action,
                "expected": expected_action,
                "correct": response.action == expected_action and response.is_valid
            })
        
        # Print summary
        print(f"\n\n{'='*70}")
        print("TEST SUMMARY")
        print(f"{'='*70}")
        
        passed = sum(1 for r in results if r["correct"])
        total = len(results)
        
        for result in results:
            status = "✓ PASS" if result["correct"] else "✗ FAIL"
            print(f"{status} | {result['query'][:40]:<40} | Got: {result['action']}")
        
        print(f"\n{passed}/{total} tests passed ({100*passed/total:.1f}%)")
        
        client.close()
        
    except grpc.RpcError as e:
        print(f"❌ RPC Error: {e.code()}")
        print(f"Details: {e.details()}")
        sys.exit(1)
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
