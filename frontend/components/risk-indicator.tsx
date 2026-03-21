'use client'

import { cn } from '@/lib/utils'

interface RiskIndicatorProps {
  score: number // 1-10
  showLabel?: boolean
  size?: 'sm' | 'md' | 'lg'
}

function getRiskColor(score: number): string {
  if (score <= 3) return 'bg-risk-low'
  if (score <= 6) return 'bg-risk-medium'
  return 'bg-risk-high'
}

function getRiskLabel(score: number): string {
  if (score <= 3) return 'Low Risk'
  if (score <= 6) return 'Medium Risk'
  return 'High Risk'
}

export function RiskIndicator({ score, showLabel = true, size = 'md' }: RiskIndicatorProps) {
  const percentage = (score / 10) * 100

  const heightClass = {
    sm: 'h-1.5',
    md: 'h-2',
    lg: 'h-3'
  }[size]

  return (
    <div className="space-y-1.5">
      {showLabel && (
        <div className="flex items-center justify-between text-sm">
          <span className="text-muted-foreground">Risk Score</span>
          <span className={cn(
            "font-medium",
            score <= 3 && "text-risk-low",
            score > 3 && score <= 6 && "text-risk-medium",
            score > 6 && "text-risk-high"
          )}>
            {score}/10 - {getRiskLabel(score)}
          </span>
        </div>
      )}
      <div className={cn("w-full rounded-full bg-muted overflow-hidden", heightClass)}>
        <div
          className={cn("h-full rounded-full transition-all", getRiskColor(score))}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  )
}
