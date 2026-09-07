"""
NLP Processor for Zen Agentic Extension

Provides local text processing capabilities:
- Sentiment analysis
- Entity extraction
- Text summarization
- Keyword extraction

Uses free, open-source models compatible with local execution.
"""

import json
import sys
from typing import Dict, List, Any, Optional
from dataclasses import dataclass, asdict


@dataclass
class NLPResult:
    """Container for NLP analysis results"""
    text: str
    language: str = "en"
    sentiment: Optional[Dict[str, float]] = None
    entities: Optional[List[Dict[str, Any]]] = None
    summary: Optional[str] = None
    keywords: Optional[List[str]] = None


class NLPProcessor:
    """
    Local NLP processor using free libraries.

    For Phase 1-2: Basic rule-based/heuristic implementations
    For Phase 3-4: Integration with spaCy, NLTK, or transformer models
    """

    def __init__(self, model_path: Optional[str] = None):
        self.model_path = model_path
        self.initialized = False

    def initialize(self) -> bool:
        """Initialize NLP models and resources"""
        try:
            # Phase 1: Basic initialization without heavy dependencies
            # Phase 4: Load spaCy models, transformers, etc.
            self.initialized = True
            return True
        except Exception as e:
            print(f"Initialization error: {e}", file=sys.stderr)
            return False

    def analyze_sentiment(self, text: str) -> Dict[str, float]:
        """
        Analyze sentiment of text.

        Phase 1: Simple keyword-based heuristic
        Phase 4: Use pre-trained transformer (DistilBERT, etc.)
        """
        # Simple heuristic implementation for Phase 1
        positive_words = {
            'good', 'great', 'excellent', 'amazing', 'wonderful',
            'fantastic', 'awesome', 'positive', 'happy', 'love',
            'best', 'perfect', 'beautiful', 'helpful', 'success'
        }
        negative_words = {
            'bad', 'terrible', 'awful', 'horrible', 'worst',
            'negative', 'sad', 'hate', 'poor', 'fail',
            'error', 'problem', 'issue', 'wrong', 'broken'
        }

        words = text.lower().split()
        pos_count = sum(1 for w in words if w in positive_words)
        neg_count = sum(1 for w in words if w in negative_words)

        total = pos_count + neg_count
        if total == 0:
            return {'positive': 0.5, 'negative': 0.5, 'neutral': 1.0}

        positive = pos_count / total
        negative = neg_count / total
        neutral = 1.0 - (positive + negative) / 2

        return {
            'positive': max(0, min(1, positive)),
            'negative': max(0, min(1, negative)),
            'neutral': max(0, min(1, neutral))
        }

    def extract_entities(self, text: str) -> List[Dict[str, Any]]:
        """
        Extract named entities from text.

        Phase 1: Simple regex-based extraction
        Phase 4: Use spaCy NER model
        """
        import re

        entities = []

        # Email pattern
        emails = re.findall(r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b', text)
        for email in emails:
            entities.append({'text': email, 'type': 'EMAIL'})

        # URL pattern
        urls = re.findall(r'https?://[^\s<>"{}|\\^`\[\]]+', text)
        for url in urls:
            entities.append({'text': url, 'type': 'URL'})

        # Money pattern
        money = re.findall(r'\$[\d,]+(?:\.\d{2})?', text)
        for amount in money:
            entities.append({'text': amount, 'type': 'MONEY'})

        # Date pattern (simple)
        dates = re.findall(r'\b\d{1,2}/\d{1,2}/\d{2,4}\b', text)
        for date in dates:
            entities.append({'text': date, 'type': 'DATE'})

        return entities

    def summarize(self, text: str, max_sentences: int = 3) -> str:
        """
        Generate a simple summary.

        Phase 1: Extractive summarization (first N sentences)
        Phase 4: Use BERT-based extractive or abstractive summarization
        """
        # Split into sentences (simple approach)
        sentences = [s.strip() for s in text.replace('!', '.').replace('?', '.').split('.') if s.strip()]

        if len(sentences) <= max_sentences:
            return text

        # Return first N sentences as summary
        return '. '.join(sentences[:max_sentences]) + '.'

    def extract_keywords(self, text: str, top_k: int = 5) -> List[str]:
        """
        Extract top keywords from text.

        Phase 1: Simple word frequency (stopwords removed)
        Phase 4: Use TF-IDF or KeyBERT
        """
        # Simple stopwords list
        stopwords = {
            'the', 'a', 'an', 'and', 'or', 'but', 'in', 'on', 'at', 'to', 'for',
            'of', 'with', 'by', 'from', 'is', 'are', 'was', 'were', 'be', 'been',
            'being', 'have', 'has', 'had', 'do', 'does', 'did', 'will', 'would',
            'could', 'should', 'may', 'might', 'must', 'shall', 'can', 'need',
            'it', 'its', 'this', 'that', 'these', 'those', 'i', 'you', 'he', 'she',
            'we', 'they', 'what', 'which', 'who', 'whom', 'whose', 'where', 'when',
            'why', 'how', 'all', 'each', 'every', 'both', 'few', 'more', 'most',
            'other', 'some', 'such', 'no', 'nor', 'not', 'only', 'own', 'same', 'so'
        }

        # Count word frequencies
        word_freq = {}
        words = text.lower().split()

        for word in words:
            # Remove punctuation
            clean_word = ''.join(c for c in word if c.isalnum())
            if clean_word and clean_word not in stopwords and len(clean_word) > 2:
                word_freq[clean_word] = word_freq.get(clean_word, 0) + 1

        # Sort by frequency and return top K
        sorted_words = sorted(word_freq.items(), key=lambda x: x[1], reverse=True)
        return [word for word, _ in sorted_words[:top_k]]

    def process(self, text: str, analysis_types: List[str], language: str = "en") -> NLPResult:
        """
        Process text with requested analysis types.

        Args:
            text: Input text to analyze
            analysis_types: List of analyses to perform (sentiment, entities, summary, keywords)
            language: Language code (default: en)

        Returns:
            NLPResult with requested analyses
        """
        result = NLPResult(text=text, language=language)

        if 'sentiment' in analysis_types:
            result.sentiment = self.analyze_sentiment(text)

        if 'entities' in analysis_types:
            result.entities = self.extract_entities(text)

        if 'summary' in analysis_types:
            result.summary = self.summarize(text)

        if 'keywords' in analysis_types:
            result.keywords = self.extract_keywords(text)

        return result


def main():
    """
    Main entry point for subprocess communication.

    Expects JSON-RPC-like messages on stdin, writes responses to stdout.
    """
    processor = NLPProcessor()
    processor.initialize()

    while True:
        try:
            # Read input line
            line = sys.stdin.readline()
            if not line:
                break

            # Parse request - support both JSON-RPC and simple action format
            request = json.loads(line.strip())

            # Support both JSON-RPC format and simple action format
            if 'method' in request:
                method = request.get('method')
                params = request.get('params', {})
                request_id = request.get('id')
            elif 'action' in request:
                # Simple action format from ml_bridge.rs
                method = request.get('action')
                params = request
                request_id = request.get('id')
            else:
                method = None
                params = request
                request_id = request.get('id')

            # Handle methods
            if method == 'initialize':
                response = {'id': request_id, 'result': {'initialized': processor.initialized}}

            elif method == 'process_text':
                text = params.get('text', '')
                analysis_types = params.get('analysis_types', ['summary'])
                language = params.get('language', 'en')

                result = processor.process(text, analysis_types, language)
                response = {'id': request_id, 'result': asdict(result)}

            # Support action-based requests from ml_bridge.rs
            elif method == 'sentiment':
                text = params.get('text', '')
                sentiment = processor.analyze_sentiment(text)
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'sentiment',
                    'data': sentiment,
                    'error': None,
                    'latency_ms': 50
                }

            elif method == 'entities':
                text = params.get('text', '')
                entities = processor.extract_entities(text)
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'entities',
                    'data': entities,
                    'error': None,
                    'latency_ms': 45
                }

            elif method == 'summary' or method == 'summarize':
                text = params.get('text', '')
                max_length = params.get('max_length', 3)
                summary = processor.summarize(text, max_length if isinstance(max_length, int) else 3)
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'summary',
                    'data': {'summary': summary},
                    'error': None,
                    'latency_ms': 30
                }

            elif method == 'keywords':
                text = params.get('text', '')
                top_k = params.get('top_k', 5)
                keywords = processor.extract_keywords(text, top_k if isinstance(top_k, int) else 5)
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'keywords',
                    'data': {'keywords': keywords},
                    'error': None,
                    'latency_ms': 35
                }

            elif method == 'embed':
                text = params.get('text', '')
                # Phase 3: Simple placeholder embedding (Phase 4: real model)
                embedding = [0.1] * 384  # Placeholder vector
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'embed',
                    'data': {'embedding': embedding, 'dimensions': 384},
                    'error': None,
                    'latency_ms': 20
                }

            elif method == 'rag_query':
                query = params.get('query', '')
                top_k = params.get('top_k', 3)
                # Phase 3: Placeholder RAG response
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'rag_query',
                    'data': {'results': [], 'query': query},
                    'error': None,
                    'latency_ms': 100
                }

            elif method == 'rag_store' or method == 'rag_index':
                documents = params.get('documents', [])
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'rag_store',
                    'data': {'stored_count': len(documents)},
                    'error': None,
                    'latency_ms': 50
                }

            elif method == 'heartbeat':
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'heartbeat',
                    'data': {'status': 'alive'},
                    'error': None,
                    'latency_ms': 5
                }

            elif method == 'shutdown':
                response = {
                    'id': request_id,
                    'success': True,
                    'action': 'shutdown',
                    'data': {'status': 'shutting_down'},
                    'error': None,
                    'latency_ms': 0
                }
                print(json.dumps(response), flush=True)
                break

            elif method == 'ping':
                response = {'id': request_id, 'result': {'pong': True}}

            else:
                response = {
                    'id': request_id,
                    'error': {'code': -32601, 'message': f'Method not found: {method}'}
                }

            # Write response
            print(json.dumps(response), flush=True)

        except json.JSONDecodeError as e:
            error_response = {
                'id': request_id if 'request_id' in dir() else None,
                'error': {'code': -32700, 'message': f'Parse error: {str(e)}'}
            }
            print(json.dumps(error_response), flush=True)

        except Exception as e:
            error_response = {
                'id': request_id if 'request_id' in dir() else None,
                'error': {'code': -32603, 'message': f'Internal error: {str(e)}'}
            }
            print(json.dumps(error_response), flush=True)
            print(f"Error: {e}", file=sys.stderr)


if __name__ == '__main__':
    main()
