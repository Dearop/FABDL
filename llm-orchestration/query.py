#!/usr/bin/env python3
"""
Command-line query tool for Intent Router.
Run from terminal: python query.py "your query here" [--output file.txt] [--json]
"""

import sys
import os
import json

# Add src directory to path so we can import generated proto files
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'src'))

import grpc
import intent_router_pb2
import intent_router_pb2_grpc


def response_to_dict(response) -> dict:
    """Convert protobuf response to dictionary"""
    return {
        "action": response.action,
        "scope": response.scope,
        "confidence": round(response.confidence, 2),
        "is_valid": response.is_valid,
        "parameters": [{p.key: p.value} for p in response.parameters]
    }


def format_response(query: str, response) -> str:
    """Format response as string"""
    output = "\n" + "="*70 + "\n"
    output += f"Query: {query}\n"
    output += "="*70 + "\n"
    output += f"Action:     {response.action}\n"
    output += f"Scope:      {response.scope}\n"
    output += f"Confidence: {response.confidence:.0%}\n"
    output += f"Valid:      {response.is_valid}\n"
    
    if response.parameters:
        output += "Parameters:\n"
        for param in response.parameters:
            output += f"  - {param.key}: {param.value}\n"
    
    output += "="*70 + "\n"
    return output


def main():
    # Parse arguments
    output_file = None
    show_json = False
    
    if len(sys.argv) < 2:
        print("Usage: python query.py 'your query here' [--output file.txt] [--json]")
        print("\nExamples:")
        print("  python query.py 'Analyze my portfolio risk'")
        print("  python query.py 'Check position' --output results.txt")
        print("  python query.py 'Analyze risk' --json")
        sys.exit(1)
    
    # Parse flags
    query = " ".join([arg for arg in sys.argv[1:] if not arg.startswith("--")])
    
    if "--output" in sys.argv:
        idx = sys.argv.index("--output")
        if idx + 1 < len(sys.argv):
            output_file = sys.argv[idx + 1]
    
    if "--json" in sys.argv:
        show_json = True
    
    try:
        # Connect to gRPC service
        channel = grpc.insecure_channel('localhost:50051')
        stub = intent_router_pb2_grpc.IntentRouterStub(channel)
        
        # Send request
        request = intent_router_pb2.IntentRequest(user_query=query)
        response = stub.ClassifyIntent(request)
        
        # Show JSON if requested
        if show_json:
            response_dict = response_to_dict(response)
            json_output = json.dumps(response_dict, indent=2)
            print("\n" + "="*70)
            print("JSON Response (sent to backend):")
            print("="*70)
            print(json_output)
            print("="*70 + "\n")
        
        # Format and print response
        output = format_response(query, response)
        print(output)
        
        # Save to file if specified
        if output_file:
            with open(output_file, 'a') as f:
                if show_json:
                    f.write("JSON Response (sent to backend):\n")
                    f.write("="*70 + "\n")
                    f.write(json_output + "\n")
                    f.write("="*70 + "\n\n")
                f.write(output)
            print(f"✓ Results saved to {output_file}")
        
    except grpc.RpcError as e:
        error_msg = f"❌ Connection Error: {e.details()}\n"
        print(error_msg)
        if output_file:
            with open(output_file, 'a') as f:
                f.write(error_msg)
        sys.exit(1)
    except Exception as e:
        error_msg = f"❌ Error: {e}\n"
        print(error_msg)
        if output_file:
            with open(output_file, 'a') as f:
                f.write(error_msg)
        sys.exit(1)


if __name__ == "__main__":
    main()
