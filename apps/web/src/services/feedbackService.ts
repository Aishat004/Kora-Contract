import type {
  FeedbackIdea,
  FeedbackSubmissionPayload,
  TriageDecisionPayload,
  TriageCadenceInfo,
  FeedbackStatus,
} from '../types/feedback';

const STORAGE_KEY = 'kora_community_feedback_ideas';
const RATE_LIMIT_KEY = 'kora_feedback_rate_limit';
const RATE_LIMIT_WINDOW_MS = 60 * 1000; // 1 minute window
const MAX_SUBMISSIONS_PER_WINDOW = 3;

export class FeedbackService {
  private ideas: FeedbackIdea[] = [];

  constructor() {
    this.loadFromStorage();
    if (this.ideas.length === 0) {
      this.seedInitialIdeas();
    }
  }

  private loadFromStorage(): void {
    if (typeof window !== 'undefined' && window.localStorage) {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        try {
          this.ideas = JSON.parse(stored);
        } catch {
          this.ideas = [];
        }
      }
    }
  }

  private saveToStorage(): void {
    if (typeof window !== 'undefined' && window.localStorage) {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.ideas));
    }
  }

  private checkRateLimit(author: string): boolean {
    if (typeof window === 'undefined' || !window.localStorage) return true;
    const now = Date.now();
    const recordsStr = localStorage.getItem(RATE_LIMIT_KEY);
    let records: { author: string; timestamp: number }[] = [];
    if (recordsStr) {
      try {
        records = JSON.parse(recordsStr);
      } catch {
        records = [];
      }
    }

    records = records.filter(r => now - r.timestamp < RATE_LIMIT_WINDOW_MS);
    const authorCount = records.filter(r => r.author === author).length;

    if (authorCount >= MAX_SUBMISSIONS_PER_WINDOW) {
      return false;
    }

    records.push({ author, timestamp: now });
    localStorage.setItem(RATE_LIMIT_KEY, JSON.stringify(records));
    return true;
  }

  public getCadenceInfo(): TriageCadenceInfo {
    const nextReview = new Date();
    nextReview.setDate(nextReview.getDate() + ((4 + 7 - nextReview.getDay()) % 7 || 7)); // Next Thursday
    nextReview.setUTCHours(16, 0, 0, 0);

    return {
      nextReviewDate: nextReview.toISOString(),
      reviewFrequencyDays: 7,
      minUpvotesForAutoReview: 5,
      responsibleCommittee: 'Governance & Product Triage Working Group',
      guidelinesUrl: 'https://github.com/OpenLedger-Foundation/Kora-Contract/blob/main/docs/governance.md',
    };
  }

  public async getIdeas(statusFilter?: FeedbackStatus): Promise<FeedbackIdea[]> {
    if (statusFilter) {
      return [...this.ideas.filter(i => i.status === statusFilter)].sort((a, b) => b.upvotes - a.upvotes);
    }
    return [...this.ideas].sort((a, b) => b.upvotes - a.upvotes);
  }

  public async submitIdea(payload: FeedbackSubmissionPayload): Promise<FeedbackIdea> {
    if (!payload.title.trim() || !payload.description.trim()) {
      throw new Error('Title and description cannot be empty');
    }

    if (!this.checkRateLimit(payload.author)) {
      throw new Error('Rate limit exceeded. Please wait before submitting another idea.');
    }

    const newIdea: FeedbackIdea = {
      id: `idea-${Date.now()}-${Math.random().toString(36).substring(2, 7)}`,
      title: payload.title.trim(),
      description: payload.description.trim(),
      category: payload.category,
      author: payload.author,
      upvotes: 1,
      upvotedBy: [payload.author],
      status: 'new',
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      comments: [],
    };

    this.ideas.unshift(newIdea);
    this.saveToStorage();
    return newIdea;
  }

  public async upvoteIdea(ideaId: string, userAddress: string): Promise<FeedbackIdea> {
    const idea = this.ideas.find(i => i.id === ideaId);
    if (!idea) {
      throw new Error('Idea not found');
    }

    if (idea.upvotedBy.includes(userAddress)) {
      // Toggle upvote off
      idea.upvotedBy = idea.upvotedBy.filter(addr => addr !== userAddress);
      idea.upvotes = Math.max(0, idea.upvotes - 1);
    } else {
      // Toggle upvote on
      idea.upvotedBy.push(userAddress);
      idea.upvotes += 1;
    }

    idea.updatedAt = new Date().toISOString();
    this.saveToStorage();
    return idea;
  }

  public async triageDecision(payload: TriageDecisionPayload): Promise<FeedbackIdea> {
    const idea = this.ideas.find(i => i.id === payload.ideaId);
    if (!idea) {
      throw new Error('Idea not found');
    }

    idea.status = payload.status;
    idea.statusReason = payload.statusReason;
    idea.promotedUrl = payload.promotedUrl;
    idea.triageNotes = payload.triageNotes;
    idea.updatedAt = new Date().toISOString();

    this.saveToStorage();
    return idea;
  }

  private seedInitialIdeas(): void {
    this.ideas = [
      {
        id: 'idea-seed-1',
        title: 'Tiered tranche repayment alerts for SMEs',
        description: 'Provide webhook / automated notifications when invoices reach within 48h of maturity window.',
        category: 'sme_onboarding',
        author: 'GD5J...SEED1',
        upvotes: 12,
        upvotedBy: ['GD5J...SEED1', 'GB6K...USER2'],
        status: 'under_review',
        createdAt: new Date(Date.now() - 86400000 * 3).toISOString(),
        updatedAt: new Date().toISOString(),
        comments: [],
      },
      {
        id: 'idea-seed-2',
        title: 'Allow custom secondary market order expiry parameters',
        description: 'Currently order expiry defaults to fixed days. Support customizable expirations.',
        category: 'protocol_mechanics',
        author: 'GC3X...INVESTOR',
        upvotes: 8,
        upvotedBy: ['GC3X...INVESTOR'],
        status: 'promoted_to_proposal',
        statusReason: 'Approved by governance triage for Wave 4 RFC.',
        promotedUrl: 'https://github.com/OpenLedger-Foundation/Kora-Contract/issues/845',
        createdAt: new Date(Date.now() - 86400000 * 5).toISOString(),
        updatedAt: new Date().toISOString(),
        comments: [],
      },
    ];
  }
}

export const feedbackService = new FeedbackService();
