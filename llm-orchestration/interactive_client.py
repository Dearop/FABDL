#!/usr/bin/env python3
"""
Interactive client for Intent Router.
Type your queries and see instant classification.
"""

import grpc
import sys
import os

# Add src directory to path so we can import generated proto files
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'src'))

import intent_router_pb2
import intent_router_pb2_grpc


def main():
    """Interactive intent classification"""
    try:
        # Connect to service
        channel = grpc.insecure_channel('localhost:50051')
        stub = intent_router_pb2_grpc.IntentRouterStub(channel)
        
        print("=" * 70)
        print("Intent Router - Interactive Mode")
        print("=" * 70)
        print("Type your queries. Press Ctrl+C to exit.\n")
        
        while True:
            # Get user input
            query = input("Your query: ").strip()
            
            if not query:
                print("(empty query, try again)\n")
                continue
            
            # Send to service
            request = intent_router_pb2.IntentRequest(user_query=query)
            response = stub.ClassifyIntent(request)
            
            # Display response
            print("\n" + "=" * 70)
            print(f"Action:     {response.action}")
            print(f"Scope:      {response.scope}")
            print(f"Confidence: {response.confidence:.0%}")
            print(f"Valid:      {response.is_valid}")
            
            if response.parameters:
                print("Parameters:")
                for param in response.parameters:
                    print(f"  - {param.key}: {param.value}")
            
            if not response.is_valid:
                print(f"Error: {response.raw_llm_output}")
            
            print("=" * 70 + "\n")
        
    except grpc.RpcError as e:
        print(f"❌ gRPC Error: {e.code()}")
        print(f"Details: {e.details()}")
        sys.exit(1)
    except KeyboardInterrupt:
        print("\n\nGoodbye!")
        sys.exit(0)
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
