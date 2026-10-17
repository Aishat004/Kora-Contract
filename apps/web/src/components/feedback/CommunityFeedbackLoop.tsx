import React, { useState, useEffect } from 'react';
import { feedbackService } from '../../services/feedbackService';
import type {
  FeedbackIdea,
  FeedbackStatus,
  FeedbackCategory,
  TriageCadenceInfo,
} from '../../types/feedback';

interface CommunityFeedbackLoopProps {
  currentUserAddress?: string;
  isMaintainer?: boolean;
}

export const CommunityFeedbackLoop: React.FC<CommunityFeedbackLoopProps> = ({
  currentUserAddress = 'GBDEMO...USER',
  isMaintainer = false,
}) => {
  const [ideas, setIdeas] = useState<FeedbackIdea[]>([]);
  const [filter, setFilter] = useState<FeedbackStatus | 'all'>('all');
  const [cadence, setCadence] = useState<TriageCadenceInfo | null>(null);
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [category, setCategory] = useState<FeedbackCategory>('protocol_mechanics');
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  // Maintainer triage form state
  const [selectedIdeaId, setSelectedIdeaId] = useState<string | null>(null);
  const [triageStatus, setTriageStatus] = useState<FeedbackStatus>('under_review');
  const [statusReason, setStatusReason] = useState('');
  const [promotedUrl, setPromotedUrl] = useState('');

  useEffect(() => {
    loadIdeas();
    setCadence(feedbackService.getCadenceInfo());
  }, [filter]);

  const loadIdeas = async () => {
    try {
      const data = await feedbackService.getIdeas(filter === 'all' ? undefined : filter);
      setIdeas(data);
    } catch (e: any) {
      setError(e.message);
    }
  };

  const handleSubmission = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccessMsg(null);
    try {
      await feedbackService.submitIdea({
        title,
        description,
        category,
        author: currentUserAddress,
      });
      setTitle('');
      setDescription('');
      setSuccessMsg('Improvement proposal idea submitted successfully!');
      await loadIdeas();
    } catch (e: any) {
      setError(e.message);
    }
  };

  const handleUpvote = async (ideaId: string) => {
    try {
      await feedbackService.upvoteIdea(ideaId, currentUserAddress);
      await loadIdeas();
    } catch (e: any) {
      setError(e.message);
    }
  };

  const handleTriageSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedIdeaId) return;
    try {
      await feedbackService.triageDecision({
        ideaId: selectedIdeaId,
        status: triageStatus,
        statusReason,
        promotedUrl,
        triagedBy: currentUserAddress,
      });
      setSelectedIdeaId(null);
      setStatusReason('');
      setPromotedUrl('');
      setSuccessMsg('Idea successfully triaged.');
      await loadIdeas();
    } catch (e: any) {
      setError(e.message);
    }
  };

  const getStatusBadgeClass = (status: FeedbackStatus) => {
    switch (status) {
      case 'new': return 'badge-new';
      case 'under_review': return 'badge-review';
      case 'promoted_to_proposal': return 'badge-promoted-proposal';
      case 'promoted_to_issue': return 'badge-promoted-issue';
      case 'declined': return 'badge-declined';
    }
  };

  return (
    <div className="community-feedback-tool p-6 max-w-5xl mx-auto">
      <header className="mb-8 border-b pb-4">
        <h1 className="text-2xl font-bold">Protocol Improvement Ideas & Feedback</h1>
        <p className="text-gray-600">
          A transparent, low-friction channel for SMEs, investors, and contributors to surface feedback before formal governance proposals.
        </p>
      </header>

      {cadence && (
        <section className="bg-blue-50 border border-blue-200 rounded p-4 mb-6">
          <h2 className="text-sm font-semibold text-blue-900 uppercase tracking-wide">Published Triage Cadence</h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mt-2 text-sm text-blue-800">
            <div>
              <span className="font-medium">Cadence:</span> Every {cadence.reviewFrequencyDays} days
            </div>
            <div>
              <span className="font-medium">Next Review Session:</span> {new Date(cadence.nextReviewDate).toLocaleDateString()}
            </div>
            <div>
              <span className="font-medium">Auto-Review Threshold:</span> {cadence.minUpvotesForAutoReview}+ upvotes
            </div>
          </div>
        </section>
      )}

      {error && <div className="p-3 bg-red-100 text-red-700 rounded mb-4">{error}</div>}
      {successMsg && <div className="p-3 bg-green-100 text-green-700 rounded mb-4">{successMsg}</div>}

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {/* Submission Form */}
        <div className="md:col-span-1 bg-white p-4 border rounded shadow-sm">
          <h2 className="font-semibold text-lg mb-3">Submit Feedback Idea</h2>
          <form onSubmit={handleSubmission} className="space-y-4">
            <div>
              <label className="block text-sm font-medium mb-1">Title</label>
              <input
                type="text"
                className="w-full border rounded p-2 text-sm"
                placeholder="Brief summary of improvement..."
                value={title}
                onChange={e => setTitle(e.target.value)}
                required
              />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">Category</label>
              <select
                className="w-full border rounded p-2 text-sm"
                value={category}
                onChange={e => setCategory(e.target.value as FeedbackCategory)}
              >
                <option value="protocol_mechanics">Protocol Mechanics</option>
                <option value="developer_experience">Developer Experience</option>
                <option value="governance">Governance</option>
                <option value="sme_onboarding">SME Onboarding</option>
                <option value="investor_tools">Investor Tools</option>
                <option value="security">Security</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">Description & Context</label>
              <textarea
                className="w-full border rounded p-2 text-sm h-28"
                placeholder="What problem does this address? Why would it benefit the protocol?"
                value={description}
                onChange={e => setDescription(e.target.value)}
                required
              />
            </div>
            <button
              type="submit"
              className="w-full bg-indigo-600 text-white py-2 px-4 rounded text-sm font-medium hover:bg-indigo-700 transition"
            >
              Submit Idea
            </button>
          </form>
        </div>

        {/* Ideas Feed */}
        <div className="md:col-span-2">
          <div className="flex justify-between items-center mb-4">
            <h2 className="font-semibold text-lg">Community Submissions ({ideas.length})</h2>
            <div className="flex items-center space-x-2">
              <label className="text-xs text-gray-500">Filter status:</label>
              <select
                className="border rounded p-1 text-xs"
                value={filter}
                onChange={e => setFilter(e.target.value as any)}
              >
                <option value="all">All</option>
                <option value="new">New</option>
                <option value="under_review">Under Review</option>
                <option value="promoted_to_proposal">Promoted to Proposal</option>
                <option value="promoted_to_issue">Promoted to Issue</option>
                <option value="declined">Declined</option>
              </select>
            </div>
          </div>

          <div className="space-y-4">
            {ideas.map(idea => {
              const hasUpvoted = idea.upvotedBy.includes(currentUserAddress);
              return (
                <div key={idea.id} className="border rounded-lg p-4 bg-white shadow-sm flex flex-col justify-between">
                  <div className="flex justify-between items-start">
                    <div className="flex-1">
                      <div className="flex items-center space-x-2">
                        <span className={`text-xs px-2 py-0.5 rounded font-medium ${getStatusBadgeClass(idea.status)}`}>
                          {idea.status.replace(/_/g, ' ').toUpperCase()}
                        </span>
                        <span className="text-xs text-gray-400 capitalize">
                          {idea.category.replace(/_/g, ' ')}
                        </span>
                      </div>
                      <h3 className="text-base font-semibold mt-1">{idea.title}</h3>
                      <p className="text-sm text-gray-600 mt-1">{idea.description}</p>
                      
                      {idea.statusReason && (
                        <div className="mt-2 text-xs bg-gray-50 p-2 rounded border-l-2 border-indigo-500">
                          <span className="font-semibold">Triage Note:</span> {idea.statusReason}
                        </div>
                      )}

                      {idea.promotedUrl && (
                        <div className="mt-2 text-xs">
                          <a
                            href={idea.promotedUrl}
                            target="_blank"
                            rel="noreferrer"
                            className="text-indigo-600 hover:underline font-medium"
                          >
                            🔗 View Promoted Formal Proposal / Issue →
                          </a>
                        </div>
                      )}
                    </div>

                    <div className="ml-4 flex flex-col items-center">
                      <button
                        onClick={() => handleUpvote(idea.id)}
                        className={`px-3 py-1.5 rounded border text-sm font-semibold flex items-center space-x-1 ${
                          hasUpvoted
                            ? 'bg-indigo-50 border-indigo-500 text-indigo-700'
                            : 'bg-white border-gray-300 text-gray-700 hover:bg-gray-50'
                        }`}
                      >
                        <span>▲</span>
                        <span>{idea.upvotes}</span>
                      </button>
                    </div>
                  </div>

                  <div className="mt-4 pt-2 border-t flex justify-between items-center text-xs text-gray-400">
                    <div>Submitted by {idea.author.slice(0, 8)}... on {new Date(idea.createdAt).toLocaleDateString()}</div>
                    {isMaintainer && (
                      <button
                        onClick={() => setSelectedIdeaId(idea.id)}
                        className="text-xs text-indigo-600 hover:underline"
                      >
                        Triage Decision
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>

          {/* Triage Modal / Panel for Maintainers */}
          {selectedIdeaId && isMaintainer && (
            <div className="fixed inset-0 bg-black bg-opacity-40 flex items-center justify-center p-4">
              <div className="bg-white rounded-lg p-6 max-w-md w-full shadow-lg">
                <h3 className="text-lg font-bold mb-4">Record Triage Decision</h3>
                <form onSubmit={handleTriageSubmit} className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium mb-1">Status</label>
                    <select
                      className="w-full border rounded p-2 text-sm"
                      value={triageStatus}
                      onChange={e => setTriageStatus(e.target.value as FeedbackStatus)}
                    >
                      <option value="under_review">Under Review</option>
                      <option value="promoted_to_proposal">Promoted to Proposal</option>
                      <option value="promoted_to_issue">Promoted to Issue</option>
                      <option value="declined">Declined</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium mb-1">Reason / Notes</label>
                    <textarea
                      className="w-full border rounded p-2 text-sm"
                      placeholder="Explain the review conclusion..."
                      value={statusReason}
                      onChange={e => setStatusReason(e.target.value)}
                      required
                    />
                  </div>
                  <div>
                    <label className="block text-sm font-medium mb-1">Promotion URL (if applicable)</label>
                    <input
                      type="url"
                      className="w-full border rounded p-2 text-sm"
                      placeholder="https://..."
                      value={promotedUrl}
                      onChange={e => setPromotedUrl(e.target.value)}
                    />
                  </div>
                  <div className="flex justify-end space-x-2">
                    <button
                      type="button"
                      onClick={() => setSelectedIdeaId(null)}
                      className="px-4 py-2 border rounded text-sm"
                    >
                      Cancel
                    </button>
                    <button
                      type="submit"
                      className="px-4 py-2 bg-indigo-600 text-white rounded text-sm"
                    >
                      Save Decision
                    </button>
                  </div>
                </form>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
